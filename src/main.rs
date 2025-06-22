use std::{fs, collections::HashMap, path::{Path as StdPath, PathBuf}};
use syn::{parse_file, visit::Visit, FnArg, Pat, Type, Item, PathArguments, GenericArgument};
use quote::ToTokens;
use serde_json::{json, Value};
use clap::Parser;
use regex::Regex;

// Add necessary imports for axum and model types
use axum::{
    routing::{get, post},
    Json, Router,
    extract::Path as AxumPath,
};

mod form;
mod response;
mod types;

use form::*;
use response::*;
use types::*;

#[derive(Parser)]
#[command(name = "axum_doc")]
#[command(about = "Generate OpenAPI documentation from Axum Rust code")]
struct Args {
    /// Base directory of the project
    #[arg(short, long, default_value = ".")]
    base_dir: String,
    
    /// Path to the handler file relative to base directory
    #[arg(short = 'f', long, default_value = "src/main.rs")]
    handler_file: String,
    
    /// Comma-separated list of model files relative to base directory
    #[arg(short, long, default_value = "src/form.rs,src/response.rs,src/types.rs")]
    model_files: String,
    
    /// Output file for the generated OpenAPI spec
    #[arg(short, long, default_value = "openapi-bak.json")]
    output: String,
}

#[derive(Debug)]
struct RouteInfo {
    path: String,
    method: String,
    handler: String,
    module: Option<String>, // 模块名，用于分组
    base_path: String,      // 基础路径，用于嵌套路由
}

struct HandlerInfo {
    params: Vec<Extractor>,
    return_type: Option<Type>,
    description: Option<String>, // 添加描述字段
}

struct Extractor {
    kind: String, // "Json", "Query", etc.
    inner_type: Type,
}

#[derive(Debug)]
struct StructInfo {
    name: String,
    fields: Vec<FieldInfo>,
}

#[derive(Debug)]
struct FieldInfo {
    name: String,
    ty: String,
}

struct RouterVisitor {
    routes: Vec<RouteInfo>,
    state_stack: Vec<(String, Option<String>)>, // (base_path, module_name)
    base_path: PathBuf, // 添加基础路径用于构建模块文件路径
}

impl<'ast> Visit<'ast> for RouterVisitor {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        // 先递归访问receiver（链式调用的左侧）
        syn::visit::visit_expr(self, &call.receiver);
        
        // 处理当前方法调用
        match call.method.to_string().as_str() {
            "route" => {
                // 处理 .route() 调用
                if let (Some(path), Some(method), Some(handler)) = (
                    parse_string_arg(&call.args[0]),
                    parse_method(&call.args[1]),
                    parse_handler_name(&call.args[1]),
                ) {
                    // 获取当前状态
                    let (current_base_path, current_module) = self.state_stack.last()
                        .map(|(bp, m)| (bp.clone(), m.clone()))
                        .unwrap_or((String::new(), None));
                    
                    // 构建完整路径
                    let full_path = if current_base_path.is_empty() {
                        path
                    } else {
                        if path.starts_with('/') {
                            format!("{}{}", current_base_path, path)
                        } else {
                            format!("{}/{}", current_base_path, path)
                        }
                    };
                    
                    println!("DEBUG: Found route - path: {}, method: {}, handler: {}, module: {:?}", 
                             full_path, method, handler, current_module);
                    
                    self.routes.push(RouteInfo {
                        path: full_path,
                        method,
                        handler,
                        module: current_module,
                        base_path: current_base_path,
                    });
                }
            }
            "nest" => {
                // 处理 .nest() 调用
                if let (Some(base_path), Some(module_name)) = (
                    parse_string_arg(&call.args[0]),
                    parse_nest_handler(&call.args[1]),
                ) {
                    println!("DEBUG: Found nest - base_path: {}, module: {}", base_path, module_name);
                    
                    // 获取当前状态
                    let current_base_path = self.state_stack.last()
                        .map(|(bp, _)| bp.clone())
                        .unwrap_or(String::new());
                    
                    // 计算新的基础路径
                    let new_base_path = if current_base_path.is_empty() {
                        base_path
                    } else {
                        format!("{}{}", current_base_path, base_path)
                    };
                    
                    println!("DEBUG: Pushing state - base_path: {}, module: {}", new_base_path, module_name);
                    
                    // 将新状态压入栈
                    self.state_stack.push((new_base_path, Some(module_name.clone())));
                    
                    // 只递归 router 函数体
                    let module_file_path = self.base_path.join(format!("src/{}/handlers.rs", module_name));
                    if module_file_path.exists() {
                        if let Ok(module_content) = fs::read_to_string(&module_file_path) {
                            if let Ok(module_ast) = parse_file(&module_content) {
                                for item in &module_ast.items {
                                    if let syn::Item::Fn(func) = item {
                                        if func.sig.ident == "router" {
                                            if let Some(syn::Stmt::Expr(expr, _)) = func.block.stmts.last() {
                                                self.visit_expr(expr);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        eprintln!("Warning: Module file not found: {}", module_file_path.display());
                    }
                    
                    // 恢复状态
                    self.state_stack.pop();
                }
            }
            _ => {
                // 其他方法调用，继续递归访问参数
                for arg in &call.args {
                    syn::visit::visit_expr(self, arg);
                }
            }
        }
        
        // 重要：不要return，继续处理后续的链式调用
    }
}

fn parse_string_arg(expr: &syn::Expr) -> Option<String> {
    if let syn::Expr::Lit(lit) = expr {
        if let syn::Lit::Str(s) = &lit.lit {
            return Some(s.value());
        }
    }
    None
}

fn parse_method(expr: &syn::Expr) -> Option<String> {
    if let syn::Expr::Call(call) = expr {
        if let syn::Expr::Path(path) = &*call.func {
            return Some(path.path.segments.last()?.ident.to_string());
        }
    }
    None
}

fn parse_handler_name(expr: &syn::Expr) -> Option<String> {
    if let syn::Expr::Call(call) = expr {
        if let Some(syn::Expr::Path(path)) = call.args.first() {
            return Some(path.path.segments.last()?.ident.to_string());
        }
    }
    None
}

fn parse_nest_handler(expr: &syn::Expr) -> Option<String> {
    // 尝试解析 nest("/path", module::handlers::router()) 中的 module
    if let syn::Expr::Call(call) = expr {
        if let syn::Expr::Path(path) = &*call.func {
            // 获取路径的第一个段作为模块名（通常是 module::handlers::router）
            if let Some(segment) = path.path.segments.first() {
                return Some(segment.ident.to_string());
            }
        }
    }
    
    // 尝试解析 nest("/path", module::handlers::router) 中的 module
    if let syn::Expr::Path(path) = expr {
        if let Some(segment) = path.path.segments.first() {
            return Some(segment.ident.to_string());
        }
    }
    
    None
}

fn parse_handler(file_content: &str, handler_name: &str) -> Option<HandlerInfo> {
    let ast = parse_file(file_content).ok()?;
    let mut handler_info = HandlerInfo {
        params: Vec::new(),
        return_type: None,
        description: None,
    };

    for item in &ast.items {
        if let Item::Fn(func) = item {
            if func.sig.ident == handler_name {
                // 提取文档注释
                let mut doc_comments = Vec::new();
                for attr in &func.attrs {
                    if attr.path().is_ident("doc") {
                        // 将属性转换为字符串并解析
                        let attr_str = attr.to_token_stream().to_string();
                        println!("DEBUG: Found doc attr: {}", attr_str);
                        if attr_str.starts_with("#[doc = ") {
                            // 提取引号内的内容
                            if let Some(start) = attr_str.find('"') {
                                if let Some(end) = attr_str.rfind('"') {
                                    if start < end {
                                        let comment = &attr_str[start + 1..end];
                                        if !comment.is_empty() {
                                            doc_comments.push(comment.to_string());
                                            println!("DEBUG: Extracted comment: {}", comment);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // 合并文档注释
                if !doc_comments.is_empty() {
                    handler_info.description = Some(doc_comments.join("\n"));
                    println!("DEBUG: Final description: {}", handler_info.description.as_ref().unwrap());
                }

                // 提取参数
                for input in &func.sig.inputs {
                    if let FnArg::Typed(pat_type) = input {
                        // 处理各种参数模式
                        match &*pat_type.pat {
                            Pat::Ident(_) => {
                                // 简单标识符模式，如 Json(payload)
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    println!("DEBUG: Found extractor - kind: {}, type: {}", kind, inner_type.to_token_stream());
                                    handler_info.params.push(Extractor {
                                        kind: kind.to_string(),
                                        inner_type,
                                    });
                                }
                            }
                            Pat::Struct(_pat_struct) => {
                                // 结构体模式，如 Path { id }
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    println!("DEBUG: Found extractor (struct) - kind: {}, type: {}", kind, inner_type.to_token_stream());
                                    handler_info.params.push(Extractor {
                                        kind: kind.to_string(),
                                        inner_type,
                                    });
                                }
                            }
                            Pat::TupleStruct(_pat_tuple) => {
                                // 元组结构体模式，如 Path(id)
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    println!("DEBUG: Found extractor (tuple) - kind: {}, type: {}", kind, inner_type.to_token_stream());
                                    handler_info.params.push(Extractor {
                                        kind: kind.to_string(),
                                        inner_type,
                                    });
                                }
                            }
                            _ => {
                                // 其他模式，尝试解析类型
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    println!("DEBUG: Found extractor (other) - kind: {}, type: {}", kind, inner_type.to_token_stream());
                                    handler_info.params.push(Extractor {
                                        kind: kind.to_string(),
                                        inner_type,
                                    });
                                }
                            }
                        }
                    }
                }

                // 提取返回类型
                if let syn::ReturnType::Type(_, ty) = &func.sig.output {
                    handler_info.return_type = Some((**ty).clone());
                }
                return Some(handler_info);
            }
        }
    }
    None
}

fn parse_extractor_type(ty: &Type) -> Option<(String, Type)> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();
            let extractors = ["Json", "Query", "Path", "Form"];

            if extractors.contains(&ident.as_str()) {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some((ident, inner_type.clone()));
                    }
                }
            }
        }
    }
    None
}

fn parse_models(file_content: &str) -> HashMap<String, StructInfo> {
    let ast = parse_file(file_content).unwrap_or_else(|_| panic!("Failed to parse model file"));
    let mut structs = HashMap::new();

    for item in ast.items {
        if let Item::Struct(item_struct) = item {
            let mut fields = Vec::new();
            if let syn::Fields::Named(named) = item_struct.fields {
                for field in named.named {
                    let ty_string = field.ty.to_token_stream().to_string();
                    fields.push(FieldInfo {
                        name: field.ident.as_ref().unwrap().to_string(),
                        ty: ty_string,
                    });
                }
            }

            structs.insert(
                item_struct.ident.to_string(),
                StructInfo {
                    name: item_struct.ident.to_string(),
                    fields,
                },
            );
        }
    }
    structs
}

fn rust_type_to_openapi(ty: &str, models: &HashMap<String, StructInfo>) -> Value {
    match ty {
        "String" | "&str" => json!({"type": "string"}),
        "i32" | "u32" | "i16" | "u16" => json!({"type": "integer", "format": "int32"}),
        "i64" | "u64" => json!({"type": "integer", "format": "int64"}),
        "f32" => json!({"type": "number", "format": "float"}),
        "f64" => json!({"type": "number", "format": "double"}),
        "bool" => json!({"type": "boolean"}),
        "Option" => json!({"type": "object"}), // 简化处理
        "Vec" => json!({"type": "array", "items": {}}),
        _ => {
            if let Some(model) = models.get(ty) {
                json!({"$ref": format!("#/components/schemas/{}", model.name)})
            } else {
                // 尝试处理泛型类型如 Vec<User>
                if let Some(inner_start) = ty.find('<') {
                    let inner_end = ty.rfind('>').unwrap_or(ty.len());
                    let inner_type = &ty[inner_start+1..inner_end];
                    if let Some(model) = models.get(inner_type) {
                        return json!({
                            "type": "array",
                            "items": {
                                "$ref": format!("#/components/schemas/{}", model.name)
                            }
                        });
                    }
                }
                json!({"type": "object"}) // 默认对象类型
            }
        }
    }
}

fn get_type_name(ty: &Type) -> String {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let mut name = segment.ident.to_string();

            // 处理包装类型如 Json<User> -> User
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(GenericArgument::Type(inner)) = args.args.first() {
                    if let Type::Path(inner_path) = inner {
                        if let Some(inner_segment) = inner_path.path.segments.last() {
                            name = inner_segment.ident.to_string();
                        }
                    }
                }
            }
            return name;
        }
    }
    ty.to_token_stream().to_string()
}

fn generate_openapi(
    routes: &[RouteInfo],
    handlers: &HashMap<String, HandlerInfo>,
    models: &HashMap<String, StructInfo>,
) -> Value {
    let mut paths = json!({});
    let mut schemas = json!({});

    // 生成模型定义
    for (_, info) in models {
        let mut properties = json!({});
        for field in &info.fields {
            properties[&field.name] = rust_type_to_openapi(&field.ty, models);
        }
        schemas[&info.name] = json!({
            "type": "object",
            "properties": properties
        });
    }

    // 生成路径定义
    for route in routes {
        if let Some(handler) = handlers.get(&route.handler) {
            let mut parameters = vec![];
            let mut request_body = None;

            // 自动补充路径参数
            let mut path_params = vec![];
            // 支持 /:id 和 /{id} 两种风格
            let mut param_names = vec![];
            // /:id
            let colon_re = Regex::new(r#":([a-zA-Z0-9_]+)"#).unwrap();
            for cap in colon_re.captures_iter(&route.path) {
                let name = cap[1].to_string();
                param_names.push(name);
            }
            // /{id}
            let brace_re = Regex::new(r#"\{([a-zA-Z0-9_]+)\}"#).unwrap();
            for cap in brace_re.captures_iter(&route.path) {
                let name = cap[1].to_string();
                param_names.push(name);
            }
            for name in param_names {
                path_params.push(json!({
                    "name": name,
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string" }
                }));
            }

            // 处理参数
            for extractor in &handler.params {
                let type_name = get_type_name(&extractor.inner_type);

                if let Some(struct_info) = models.get(&type_name) {
                    match extractor.kind.as_str() {
                        "Path" | "Query" => {
                            for field in &struct_info.fields {
                                let required = !field.ty.starts_with("Option");

                                parameters.push(json!({
                                    "name": field.name,
                                    "in": extractor.kind.to_lowercase(),
                                    "required": required,
                                    "schema": rust_type_to_openapi(&field.ty, models)
                                }));
                            }
                        }
                        "Json" | "Form" => {
                            request_body = Some(json!({
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "$ref": format!("#/components/schemas/{}", type_name)
                                        }
                                    }
                                }
                            }));
                        }
                        _ => {}
                    }
                }
            }

            // 处理响应
            let mut responses = json!({});
            if let Some(return_type) = &handler.return_type {
                let type_name = get_type_name(return_type);

                if models.contains_key(&type_name) {
                    responses["200"] = json!({
                        "description": "Successful response",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": format!("#/components/schemas/{}", type_name)
                                }
                            }
                        }
                    });
                } else {
                    // 处理基础类型或未定义类型
                    let type_str = return_type.to_token_stream().to_string();
                    responses["200"] = json!({
                        "description": "Successful response",
                        "content": {
                            "application/json": {
                                "schema": rust_type_to_openapi(&type_str, models)
                            }
                        }
                    });
                }
            }

            // 添加到路径
            let path_key = &route.path;
            
            // 确保路径存在
            if !paths.as_object().unwrap().contains_key(path_key) {
                paths[path_key] = json!({});
            }
            
            let path_entry = paths[path_key]
                .as_object_mut()
                .unwrap();

            // 构建操作对象
            let mut operation = json!({
                "summary": format!("{} {}", route.method.to_uppercase(), route.handler),
                "operationId": route.handler,
                "responses": responses
            });
            
            // 添加描述（如果存在）
            if let Some(description) = &handler.description {
                operation["description"] = json!(description);
            }
            
            // 合并路径参数和提取器参数，避免重复
            let mut all_parameters = parameters;
            let mut existing_names: std::collections::HashSet<String> = all_parameters
                .iter()
                .filter_map(|p| p.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect();
            for p in path_params {
                if let Some(name) = p.get("name").and_then(|n| n.as_str()) {
                    if !existing_names.contains(name) {
                        all_parameters.push(p);
                    }
                }
            }
            if !all_parameters.is_empty() {
                operation["parameters"] = json!(all_parameters);
            }
            
            // 只有当requestBody不为空时才添加requestBody字段
            if let Some(rb) = request_body {
                operation["requestBody"] = rb;
            }
            
            // 添加tags用于分组
            if let Some(module_name) = &route.module {
                operation["tags"] = json!([module_name]);
            }

            path_entry.insert(
                route.method.to_lowercase(),
                operation,
            );
        }
    }

    json!({
        "openapi": "3.0.0",
        "info": {
            "title": "Generated API",
            "version": "1.0.0",
            "description": "Auto-generated OpenAPI specification from Axum routes"
        },
        "paths": paths,
        "components": {
            "schemas": schemas
        }
    })
}

// Example handler functions for demonstration
/// 用户登录接口
/// 
/// 接收用户名和密码，返回用户资料信息
async fn login(Json(payload): Json<UserLogin>) -> Json<UserProfile> {
    Json(UserProfile {
        id: 1,
        username: payload.username,
        email: "test@example.com".to_string(),
    })
}

/// 获取用户信息
/// 
/// 根据用户ID获取用户基本信息
async fn get_user(AxumPath(id): AxumPath<u64>) -> Json<User> {
    Json(User { id, name: "test".to_string() })
}

// 用户模块的handler函数
/// 获取用户详细资料
/// 
/// 根据用户ID获取用户的完整资料信息，包括用户名和邮箱
async fn get_user_profile(AxumPath(id): AxumPath<u64>) -> Json<UserProfile> {
    Json(UserProfile {
        id,
        username: format!("user_{}", id),
        email: format!("user{}@example.com", id),
    })
}

/// 更新用户信息
/// 
/// 根据用户ID更新用户的基本信息
async fn update_user(AxumPath(id): AxumPath<u64>, Json(user): Json<User>) -> Json<User> {
    Json(User { id, name: user.name })
}

fn app() -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/user/:id", get(get_user))
        .nest("/api", user::router())
        .nest("/test", test::router())
        .nest("/events", event::router())
}

// 添加用户模块
mod user {
    use axum::{
        routing::{get, put},
        Router,
    };
    
    pub fn router() -> Router {
        Router::new()
            .route("/profile/:id", get(super::get_user_profile))
            .route("/:id", put(super::update_user))
    }
}

// 添加测试模块
mod test {
    use axum::{
        routing::get,
        Router,
    };
    
    pub fn router() -> Router {
        Router::new()
            .route("/status", get(super::test_status))
    }
}

// 添加事件模块
mod event {
    use axum::{
        routing::get,
        Router,
    };
    
    pub fn router() -> Router {
        Router::new()
            .route("/list", get(super::event_list))
    }
}

// 添加测试和事件模块的handler函数
/// 获取系统状态
/// 
/// 返回系统运行状态信息
async fn test_status() -> &'static str {
    "OK"
}

/// 获取事件列表
/// 
/// 返回所有可用事件的列表
async fn event_list() -> &'static str {
    "[]"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // 构建基础路径
    let base_path = StdPath::new(&args.base_dir);
    if !base_path.exists() {
        return Err(format!("Base directory does not exist: {}", args.base_dir).into());
    }
    
    // 构建handler文件路径
    let handler_path = base_path.join(&args.handler_file);
    if !handler_path.exists() {
        return Err(format!("Handler file does not exist: {}", handler_path.display()).into());
    }
    
    // 解析模型文件列表
    let model_files: Vec<String> = args.model_files
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    if model_files.is_empty() {
        eprintln!("Warning: No model files specified");
    }
    
    // 构建模型文件路径
    let model_paths: Vec<_> = model_files
        .iter()
        .map(|file| base_path.join(file))
        .collect();
    
    let _app = app();

    // 1. 解析路由文件
    let router_content = fs::read_to_string(&handler_path)?;
    let router_ast = parse_file(&router_content)?;

    let mut visitor = RouterVisitor { 
        routes: Vec::new(), 
        state_stack: Vec::new(), 
        base_path: base_path.to_path_buf() 
    };
    visitor.visit_file(&router_ast);

    // 2. 解析处理器函数
    let mut handlers = HashMap::new();
    let mut module_handlers = HashMap::new(); // 存储模块名到handler文件的映射
    
    for route in &visitor.routes {
        // 首先尝试从主handler文件中解析
        if let Some(handler) = parse_handler(&router_content, &route.handler) {
            handlers.insert(route.handler.clone(), handler);
        } else if let Some(module_name) = &route.module {
            // 如果主文件中没找到，尝试从模块文件中解析
            let module_handler_path = base_path.join(format!("src/{}/handlers.rs", module_name));
            
            if !module_handlers.contains_key(module_name) {
                if module_handler_path.exists() {
                    let module_content = fs::read_to_string(&module_handler_path)?;
                    module_handlers.insert(module_name.clone(), module_content);
                    println!("Found module handler file: {}", module_handler_path.display());
                } else {
                    eprintln!("Warning: Module handler file not found: {}", module_handler_path.display());
                }
            }
            
            if let Some(module_content) = module_handlers.get(module_name) {
                if let Some(handler) = parse_handler(module_content, &route.handler) {
                    handlers.insert(route.handler.clone(), handler);
                } else {
                    eprintln!("Warning: Handler '{}' not found in module '{}'", route.handler, module_name);
                }
            }
        } else {
            eprintln!("Warning: Handler '{}' not found", route.handler);
        }
    }

    // 3. 解析模型文件
    let mut all_models = HashMap::new();
    for path in &model_paths {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let models = parse_models(&content);
            all_models.extend(models);
            println!("Parsed models from: {}", path.display());
        } else {
            eprintln!("Warning: Model file not found: {}", path.display());
        }
    }

    // 4. 生成OpenAPI
    let openapi = generate_openapi(&visitor.routes, &handlers, &all_models);
    let pretty_json = serde_json::to_string_pretty(&openapi)?;
    
    // 构建输出文件路径
    let output_path = base_path.join(&args.output);
    fs::write(&output_path, pretty_json)?;

    println!("OpenAPI spec generated successfully at: {}", output_path.display());
    println!("Found {} routes", visitor.routes.len());
    println!("Found {} models", all_models.len());
    Ok(())
}