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

mod response;
mod types;

use serde::{Deserialize, Serialize};
use response::*;
use types::*;

// Example model for demonstration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLogin {
    pub username: String,
    pub password: String,
}

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
}

struct HandlerInfo {
    params: Vec<Extractor>,
    return_type: Option<Type>,
    summary: Option<String>,    // Summary from first line of doc comments
    description: Option<String>, // Description from remaining lines
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
    module_stack: Vec<String>, // Track current module path for nested modules
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
                    
                    //println!("DEBUG: Found route - path: {}, method: {}, handler: {}, module: {:?}", 
                    //          full_path, method, handler, current_module);
                    
                    self.routes.push(RouteInfo {
                        path: full_path,
                        method,
                        handler,
                        module: current_module,
                    });
                }
            }
            "nest" => {
                // 处理 .nest() 调用
                if let (Some(base_path), Some(module_name)) = (
                    parse_string_arg(&call.args[0]),
                    parse_nest_handler(&call.args[1]),
                ) {
                    //println!("DEBUG: Found nest - base_path: {}, module: {}", base_path, module_name);

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

                    //println!("DEBUG: Pushing state - base_path: {}, module: {}", new_base_path, module_name);

                    // 将新状态压入栈
                    self.state_stack.push((new_base_path, Some(module_name.clone())));

                    // Push module onto module_stack for nested resolution
                    self.module_stack.push(module_name.clone());

                    // Try multiple file patterns for the module
                    // Build nested module path if we're in a nested context
                    let module_path = if !self.module_stack.is_empty() {
                        self.module_stack.join("/")
                    } else {
                        module_name.clone()
                    };

                    let module_file_paths = vec![
                        self.base_path.join(format!("src/{}/handlers.rs", module_path)),
                        self.base_path.join(format!("src/{}/mod.rs", module_path)),
                        self.base_path.join(format!("src/{}.rs", module_path)),
                    ];

                    let mut found = false;
                    for module_file_path in module_file_paths {
                        if module_file_path.exists() {
                            if let Ok(module_content) = fs::read_to_string(&module_file_path) {
                                if let Ok(module_ast) = parse_file(&module_content) {
                                    for item in &module_ast.items {
                                        if let syn::Item::Fn(func) = item {
                                            if func.sig.ident == "router" {
                                                if let Some(syn::Stmt::Expr(expr, _)) = func.block.stmts.last() {
                                                    self.visit_expr(expr);
                                                    found = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }

                    // Pop module from stack
                    self.module_stack.pop();

                    if !found {
                        eprintln!("Warning: Module file not found for nest: {}", module_name);
                    }

                    // 恢复状态
                    self.state_stack.pop();
                }
            }
            "merge" => {
                // 处理 .merge() 调用
                // merge() 不添加路径前缀，只是合并另一个路由
                if let Some(module_name) = parse_merge_handler(&call.args[0]) {
                    //println!("DEBUG: Found merge - module: {}", module_name);

                    // 获取当前状态（merge 不改变路径前缀）
                    let (current_base_path, current_module) = self.state_stack.last()
                        .map(|(bp, m)| (bp.clone(), m.clone()))
                        .unwrap_or((String::new(), None));

                    // 将当前状态压入栈（merge 不改变前缀和模块）
                    self.state_stack.push((current_base_path.clone(), current_module));

                    // Push module onto module_stack for nested resolution
                    self.module_stack.push(module_name.clone());

                    // Try multiple file patterns for the module
                    // Build nested module path if we're in a nested context
                    let module_path = if !self.module_stack.is_empty() {
                        self.module_stack.join("/")
                    } else {
                        module_name.clone()
                    };

                    let module_file_paths = vec![
                        self.base_path.join(format!("src/{}/handlers.rs", module_path)),
                        self.base_path.join(format!("src/{}/mod.rs", module_path)),
                        self.base_path.join(format!("src/{}.rs", module_path)),
                    ];

                    let mut found = false;
                    for module_file_path in module_file_paths {
                        if module_file_path.exists() {
                            if let Ok(module_content) = fs::read_to_string(&module_file_path) {
                                if let Ok(module_ast) = parse_file(&module_content) {
                                    for item in &module_ast.items {
                                        if let syn::Item::Fn(func) = item {
                                            if func.sig.ident == "router" {
                                                if let Some(syn::Stmt::Expr(expr, _)) = func.block.stmts.last() {
                                                    self.visit_expr(expr);
                                                    found = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }

                    // Pop module from stack
                    self.module_stack.pop();

                    if !found {
                        eprintln!("Warning: Module file not found for merge: {}", module_name);
                    }

                    // 恢复状态
                    self.state_stack.pop();
                } else {
                    // 如果不是模块调用，则递归访问表达式
                    // 这处理了内联的 router 表达式，如 merge(Router::new().route(...))
                    syn::visit::visit_expr(self, &call.args[0]);
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

fn parse_merge_handler(expr: &syn::Expr) -> Option<String> {
    // Try to parse merge(module::handlers::router()) or merge(module::handlers::router)
    if let syn::Expr::Call(call) = expr {
        if let syn::Expr::Path(path) = &*call.func {
            // Get the first segment as module name
            if let Some(segment) = path.path.segments.first() {
                return Some(segment.ident.to_string());
            }
        }
    }

    // Try to parse merge(module::handlers::router) without call
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
        summary: None,
        description: None,
    };

    for item in &ast.items {
        if let Item::Fn(func) = item {
            if func.sig.ident == handler_name {
                // Extract documentation comments
                let mut doc_comments = Vec::new();

                for attr in &func.attrs {
                    if attr.path().is_ident("doc") {
                        // Handle Meta::NameValue (most common for /// comments)
                        if let syn::Meta::NameValue(nv) = &attr.meta {
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                    let content = lit_str.value().trim().to_string();
                                    if !content.is_empty() {
                                        doc_comments.push(content);
                                    }
                                }
                            }
                        }
                        // Handle Meta::List (for #![doc = "..."] style)
                        else if let syn::Meta::List(meta_list) = &attr.meta {
                            let tokens = meta_list.tokens.to_string();
                            if let Some(start) = tokens.find('"') {
                                if let Some(end) = tokens.rfind('"') {
                                    if start < end {
                                        let content = &tokens[start + 1..end];
                                        if !content.is_empty() {
                                            doc_comments.push(content.trim().to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Split into summary (first line) and description (rest)
                if !doc_comments.is_empty() {
                    handler_info.summary = Some(doc_comments[0].clone());
                    if doc_comments.len() > 1 {
                        // Filter out empty lines for description
                        let non_empty: Vec<String> = doc_comments[1..].iter()
                            .filter(|s| !s.is_empty())
                            .cloned()
                            .collect();
                        if !non_empty.is_empty() {
                            handler_info.description = Some(non_empty.join("\n"));
                        }
                    }
                }

                // 提取参数
                for input in &func.sig.inputs {
                    if let FnArg::Typed(pat_type) = input {
                        // 处理各种参数模式
                        match &*pat_type.pat {
                            Pat::Ident(_) => {
                                // 简单标识符模式，如 Json(payload)
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    //println!("DEBUG: Found extractor - kind: {}, type: {}", kind, inner_type.to_token_stream());
                                    handler_info.params.push(Extractor {
                                        kind: kind.to_string(),
                                        inner_type,
                                    });
                                }
                            }
                            Pat::Struct(_pat_struct) => {
                                // 结构体模式，如 Path { id }
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    //println!("DEBUG: Found extractor (struct) - kind: {}, type: {}", kind, inner_type.to_token_stream());
                                    handler_info.params.push(Extractor {
                                        kind: kind.to_string(),
                                        inner_type,
                                    });
                                }
                            }
                            Pat::TupleStruct(_pat_tuple) => {
                                // 元组结构体模式，如 Path(id)
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    //println!("DEBUG: Found extractor (tuple) - kind: {}, type: {}", kind, inner_type.to_token_stream());
                                    handler_info.params.push(Extractor {
                                        kind: kind.to_string(),
                                        inner_type,
                                    });
                                }
                            }
                            _ => {
                                // 其他模式，尝试解析类型
                                if let Some((kind, inner_type)) = parse_extractor_type(&pat_type.ty) {
                                    //println!("DEBUG: Found extractor (other) - kind: {}, type: {}", kind, inner_type.to_token_stream());
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
    // Handle generics first (order matters - must check before simple types)
    if let Some(inner_start) = ty.find('<') {
        let outer_type = &ty[..inner_start];
        let inner_end = ty.rfind('>').unwrap_or(ty.len());
        let inner_type = &ty[inner_start + 1..inner_end];

        match outer_type {
            "Vec" | "std::vec::Vec" => {
                return json!({
                    "type": "array",
                    "items": rust_type_to_openapi(inner_type, models)
                });
            }
            "Option" | "std::option::Option" => {
                let mut inner_schema = rust_type_to_openapi(inner_type, models);
                inner_schema["nullable"] = json!(true);
                return inner_schema;
            }
            "HashMap" | "std::collections::HashMap" => {
                let parts: Vec<&str> = inner_type.split(',').collect();
                if parts.len() == 2 {
                    let value_type = parts[1].trim();
                    return json!({
                        "type": "object",
                        "additionalProperties": rust_type_to_openapi(value_type, models)
                    });
                }
            }
            _ => {}
        }
    }

    // Handle simple types
    match ty {
        "String" | "&str" | "str" => json!({"type": "string"}),
        "i8" | "u8" | "i16" | "u16" | "i32" | "u32" => json!({"type": "integer", "format": "int32"}),
        "i64" | "u64" | "isize" | "usize" => json!({"type": "integer", "format": "int64"}),
        "f32" => json!({"type": "number", "format": "float"}),
        "f64" => json!({"type": "number", "format": "double"}),
        "bool" => json!({"type": "boolean"}),
        // UUID type (check for uuid::Uuid or just Uuid)
        t if t.contains("Uuid") => json!({
            "type": "string",
            "format": "uuid",
            "example": "550e8400-e29b-41d4-a716-446655440000"
        }),
        // DateTime types
        t if t.contains("DateTime") || t.contains("chrono") => json!({
            "type": "string",
            "format": "date-time",
            "example": "2024-01-01T00:00:00Z"
        }),
        // Duration
        t if t.contains("Duration") || t.contains("duration") => json!({
            "type": "string",
            "format": "duration"
        }),
        // Custom types from models
        _ => {
            if let Some(model) = models.get(ty) {
                json!({"$ref": format!("#/components/schemas/{}", model.name)})
            } else {
                eprintln!("Warning: Unknown type '{}', defaulting to object", ty);
                json!({"type": "object"})
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
        println!("check route: {}", route.path);
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
            // Use summary from doc comments if available, otherwise fallback to method + handler name
            let summary = handler.summary.as_ref()
                .cloned()
                .unwrap_or_else(|| format!("{} {}", route.method.to_uppercase(), route.handler));

            let mut operation = json!({
                "summary": summary,
                "operationId": route.handler,
                "responses": responses
            });

            // 添加描述（如果存在）
            if let Some(description) = &handler.description {
                operation["description"] = json!(description);
            }
            
            // 合并路径参数和提取器参数，避免重复
            let mut all_parameters = parameters;
            let existing_names: std::collections::HashSet<String> = all_parameters
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
        }else{
            println!("router:{} not found handler:{}", route.path, route.handler);
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
        base_path: base_path.to_path_buf(),
        module_stack: Vec::new(),
    };
    visitor.visit_file(&router_ast);

    // 2. 解析处理器函数
    let mut handlers = HashMap::new();
    let mut module_handlers = HashMap::new(); // 存储模块名到handler文件的映射
    
    for route in &visitor.routes {
        // 首先尝试从主handler文件中解析
        if let Some(handler) = parse_handler(&router_content, &route.handler) {
            println!("ADD Handler:{}", route.path);
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
    // println!("routes:{:?}", visitor.routes);
    // for (key, info) in handlers.iter() {
    //     println!("|-> handler:{}", key);
    // }
    // println!("{} handlers", handlers.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_rust_type_to_openapi_primitives() {
        let models = HashMap::new();

        // Test String
        let schema = rust_type_to_openapi("String", &models);
        assert_eq!(schema["type"], "string");

        // Test i32
        let schema = rust_type_to_openapi("i32", &models);
        assert_eq!(schema["type"], "integer");
        assert_eq!(schema["format"], "int32");

        // Test i64
        let schema = rust_type_to_openapi("i64", &models);
        assert_eq!(schema["type"], "integer");
        assert_eq!(schema["format"], "int64");

        // Test f32
        let schema = rust_type_to_openapi("f32", &models);
        assert_eq!(schema["type"], "number");
        assert_eq!(schema["format"], "float");

        // Test f64
        let schema = rust_type_to_openapi("f64", &models);
        assert_eq!(schema["type"], "number");
        assert_eq!(schema["format"], "double");

        // Test bool
        let schema = rust_type_to_openapi("bool", &models);
        assert_eq!(schema["type"], "boolean");
    }

    #[test]
    fn test_rust_type_to_openapi_uuid() {
        let models = HashMap::new();
        let schema = rust_type_to_openapi("Uuid", &models);
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["format"], "uuid");
        assert_eq!(schema["example"], "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_rust_type_to_openapi_datetime() {
        let models = HashMap::new();
        let schema = rust_type_to_openapi("DateTime", &models);
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["format"], "date-time");
        assert_eq!(schema["example"], "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_rust_type_to_openapi_duration() {
        let models = HashMap::new();
        let schema = rust_type_to_openapi("Duration", &models);
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["format"], "duration");
    }

    #[test]
    fn test_rust_type_to_openapi_vec() {
        let models = HashMap::new();

        // Test Vec<String>
        let schema = rust_type_to_openapi("Vec<String>", &models);
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["items"]["type"], "string");

        // Test Vec<i32>
        let schema = rust_type_to_openapi("Vec<i32>", &models);
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["items"]["type"], "integer");
        assert_eq!(schema["items"]["format"], "int32");
    }

    #[test]
    fn test_rust_type_to_openapi_option() {
        let models = HashMap::new();

        // Test Option<String>
        let schema = rust_type_to_openapi("Option<String>", &models);
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["nullable"], true);

        // Test Option<i64>
        let schema = rust_type_to_openapi("Option<i64>", &models);
        assert_eq!(schema["type"], "integer");
        assert_eq!(schema["format"], "int64");
        assert_eq!(schema["nullable"], true);
    }

    #[test]
    fn test_rust_type_to_openapi_hashmap() {
        let models = HashMap::new();

        // Test HashMap<String, i32>
        let schema = rust_type_to_openapi("HashMap<String, i32>", &models);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"]["type"], "integer");
        assert_eq!(schema["additionalProperties"]["format"], "int32");
    }

    #[test]
    fn test_rust_type_to_openapi_custom_type() {
        let mut models = HashMap::new();
        models.insert("User".to_string(), StructInfo {
            name: "User".to_string(),
            fields: vec![],
        });

        let schema = rust_type_to_openapi("User", &models);
        assert_eq!(schema["$ref"], "#/components/schemas/User");
    }

    #[test]
    fn test_parse_string_arg() {
        // Test parsing string literal
        let expr: syn::Expr = syn::parse_quote!("/api/v1/users");
        let result = parse_string_arg(&expr);
        assert_eq!(result, Some("/api/v1/users".to_string()));

        // Test parsing non-string
        let expr: syn::Expr = syn::parse_quote!(123);
        let result = parse_string_arg(&expr);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_method() {
        // Test parsing method call like post(handler)
        let expr: syn::Expr = syn::parse_quote!(post(my_handler));
        let result = parse_method(&expr);
        assert_eq!(result, Some("post".to_string()));

        // Test parsing get(handler)
        let expr: syn::Expr = syn::parse_quote!(get(my_handler));
        let result = parse_method(&expr);
        assert_eq!(result, Some("get".to_string()));
    }

    #[test]
    fn test_parse_handler_name() {
        // Test parsing handler name from method call
        let expr: syn::Expr = syn::parse_quote!(post(my_handler));
        let result = parse_handler_name(&expr);
        assert_eq!(result, Some("my_handler".to_string()));

        // Test parsing nested path (only gets the final segment)
        let expr: syn::Expr = syn::parse_quote!(post(module::handler));
        let result = parse_handler_name(&expr);
        assert_eq!(result, Some("handler".to_string()));
    }

    #[test]
    fn test_parse_nest_handler() {
        // Test parsing module::router() call
        let expr: syn::Expr = syn::parse_quote!(module::handlers::router());
        let result = parse_nest_handler(&expr);
        assert_eq!(result, Some("module".to_string()));

        // Test parsing simple path
        let expr: syn::Expr = syn::parse_quote!(module::router);
        let result = parse_nest_handler(&expr);
        assert_eq!(result, Some("module".to_string()));
    }

    #[test]
    fn test_parse_merge_handler() {
        // Test parsing module::handlers::router() call
        let expr: syn::Expr = syn::parse_quote!(module::handlers::router());
        let result = parse_merge_handler(&expr);
        assert_eq!(result, Some("module".to_string()));

        // Test parsing simple path
        let expr: syn::Expr = syn::parse_quote!(module::router);
        let result = parse_merge_handler(&expr);
        assert_eq!(result, Some("module".to_string()));
    }

    #[test]
    fn test_extract_doc_comments_from_attrs() {
        // This test verifies that doc comment extraction logic works
        // We create attributes that syn would produce from /// comments

        let code = r#"
        /// User login endpoint
        ///
        /// This endpoint handles user authentication and returns a JWT token.
        async fn test_handler() -> &'static str {
            "ok"
        }
        "#;

        let ast = parse_file(code).unwrap();
        if let syn::Item::Fn(func) = &ast.items[0] {
            let mut doc_comments = Vec::new();

            for attr in &func.attrs {
                if attr.path().is_ident("doc") {
                    if let syn::Meta::NameValue(nv) = &attr.meta {
                        if let syn::Expr::Lit(expr_lit) = &nv.value {
                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                let content = lit_str.value().trim().to_string();
                                if !content.is_empty() {
                                    doc_comments.push(content);
                                }
                            }
                        }
                    }
                }
            }

            // Blank lines are filtered out after trimming
            assert_eq!(doc_comments.len(), 2);
            assert_eq!(doc_comments[0], "User login endpoint");
            assert_eq!(doc_comments[1], "This endpoint handles user authentication and returns a JWT token.");
        }
    }

    #[test]
    fn test_doc_comment_splitting() {
        // Test splitting doc comments into summary and description

        let code = r#"
        /// Summary line
        ///
        /// Detailed description
        /// Second line of description
        async fn test_handler() -> &'static str {
            "ok"
        }
        "#;

        let ast = parse_file(code).unwrap();
        if let syn::Item::Fn(func) = &ast.items[0] {
            let mut doc_comments = Vec::new();

            for attr in &func.attrs {
                if attr.path().is_ident("doc") {
                    if let syn::Meta::NameValue(nv) = &attr.meta {
                        if let syn::Expr::Lit(expr_lit) = &nv.value {
                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                let content = lit_str.value().trim().to_string();
                                if !content.is_empty() {
                                    doc_comments.push(content);
                                }
                            }
                        }
                    }
                }
            }

            // Simulate the splitting logic
            let summary = if !doc_comments.is_empty() {
                Some(doc_comments[0].clone())
            } else {
                None
            };

            let description = if doc_comments.len() > 1 {
                // Filter out empty lines and join
                let non_empty: Vec<String> = doc_comments[1..].iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect();
                if non_empty.is_empty() {
                    None
                } else {
                    Some(non_empty.join("\n"))
                }
            } else {
                None
            };

            assert_eq!(summary, Some("Summary line".to_string()));
            assert_eq!(description, Some("Detailed description\nSecond line of description".to_string()));
        }
    }

    #[test]
    fn test_nested_generic_types() {
        let models = HashMap::new();

        // Test Vec<Vec<String>>
        let schema = rust_type_to_openapi("Vec<Vec<String>>", &models);
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["items"]["type"], "array");
        assert_eq!(schema["items"]["items"]["type"], "string");

        // Test Option<Vec<i32>>
        let schema = rust_type_to_openapi("Option<Vec<i32>>", &models);
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["nullable"], true);
        assert_eq!(schema["items"]["type"], "integer");
    }

    #[test]
    fn test_unknown_type_fallback() {
        let models = HashMap::new();

        // Test unknown type falls back to object
        let schema = rust_type_to_openapi("UnknownType", &models);
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn test_usize_isize_types() {
        let models = HashMap::new();

        // Test usize
        let schema = rust_type_to_openapi("usize", &models);
        assert_eq!(schema["type"], "integer");
        assert_eq!(schema["format"], "int64");

        // Test isize
        let schema = rust_type_to_openapi("isize", &models);
        assert_eq!(schema["type"], "integer");
        assert_eq!(schema["format"], "int64");
    }

    #[test]
    fn test_and_str_type() {
        let models = HashMap::new();

        // Test &str
        let schema = rust_type_to_openapi("&str", &models);
        assert_eq!(schema["type"], "string");
    }

    #[test]
    fn test_complex_hashmap() {
        let models = HashMap::new();

        // Test HashMap<String, Vec<i32>>
        let schema = rust_type_to_openapi("HashMap<String, Vec<i32>>", &models);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"]["type"], "array");
        assert_eq!(schema["additionalProperties"]["items"]["type"], "integer");
    }

    #[test]
    fn test_module_handler_parsing() {
        // Test that we can extract module name from various patterns

        // Pattern 1: module::handlers::router()
        let expr: syn::Expr = syn::parse_quote!(api::handlers::router());
        let result = parse_merge_handler(&expr);
        assert_eq!(result, Some("api".to_string()));

        // Pattern 2: auth::router
        let expr: syn::Expr = syn::parse_quote!(auth::router);
        let result = parse_merge_handler(&expr);
        assert_eq!(result, Some("auth".to_string()));

        // Pattern 3: single module
        let expr: syn::Expr = syn::parse_quote!(users::router());
        let result = parse_nest_handler(&expr);
        assert_eq!(result, Some("users".to_string()));
    }

    #[test]
    fn test_single_doc_comment() {
        // Test handler with only summary, no description

        let code = r#"
        /// Single line comment
        async fn test_handler() -> &'static str {
            "ok"
        }
        "#;

        let ast = parse_file(code).unwrap();
        if let syn::Item::Fn(func) = &ast.items[0] {
            let mut doc_comments = Vec::new();

            for attr in &func.attrs {
                if attr.path().is_ident("doc") {
                    if let syn::Meta::NameValue(nv) = &attr.meta {
                        if let syn::Expr::Lit(expr_lit) = &nv.value {
                            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                let content = lit_str.value().trim().to_string();
                                if !content.is_empty() {
                                    doc_comments.push(content);
                                }
                            }
                        }
                    }
                }
            }

            assert_eq!(doc_comments.len(), 1);
            // Doc comments are now trimmed
            assert_eq!(doc_comments[0], "Single line comment");
        }
    }
}
