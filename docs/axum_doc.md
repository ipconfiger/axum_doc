# axum_doc 改进建议

> 基于 ru_service_base 项目的实际测试结果，为 axum_doc 提供详细的改进建议。

## 目录

- [问题汇总](#问题汇总)
- [详细分析](#详细分析)
- [实现方案](#实现方案)
- [测试用例](#测试用例)
- [优先级建议](#优先级建议)

---

## 问题汇总

### 1. 模块化路由支持不完整

**现象**:
```bash
$ axum_doc --base-dir . --handler-file src/main.rs
Found 1 routes  # 只识别了根路由，遗漏了所有模块路由
```

**影响**: 无法从单次命令生成完整的 API 文档，需要为每个模块单独生成。

### 2. 路径前缀缺失

**现象**:
```json
// 期望: /api/v1/auth/login
// 实际: /login
{
  "paths": {
    "/login": { ... }  // 缺少 /api/v1/auth 前缀
  }
}
```

**影响**: 生成的路径不完整，需要手动添加前缀。

### 3. 类型推断不准确

**现象**:
```json
{
  "user_id": { "type": "object" },      // 应该是 "string" (UUID)
  "id": { "type": "object" },           // 应该是 "integer" (i64)
  "expires_at": { "type": "integer", "format": "int64" }  // 正确
}
```

**影响**: OpenAPI 文档类型错误，客户端代码生成不正确。

### 4. 文档注释未解析

**现象**:
```rust
/// 用户登录（返回真实 token）
///
/// POST /api/v1/auth/login
/// Body: {"username": "admin", "password": "admin123"}
async fn login(Json(form): Json<LoginForm>) -> AppResp<LoginResponse>
```

生成的 OpenAPI:
```json
{
  "summary": "POST login",  // 应该是 "用户登录（返回真实 token）"
  "description": null
}
```

**影响**: 文档缺乏描述性，降低开发体验。

---

## 详细分析

### 问题 1: 模块化路由支持不完整

#### 根本原因

**项目路由结构**:
```rust
// src/main.rs
fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .merge(modules::router())  // ← axum_doc 未追踪
        .with_state(state)
}

// src/modules/mod.rs
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())        // ← 未追踪
        .nest("/api/v1/auth", auth::router())   // ← 未追踪 + 前缀丢失
        .nest("/api/v1/user", user::router())   // ← 未追踪 + 前缀丢失
}

// src/modules/auth/handler.rs
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))   // ← 只能识别这一层
        .route("/logout", post(logout))
        .route("/verify", post(verify))
}
```

**技术分析**:

当前 axum_doc 可能只做了简单的 `Router::route()` 匹配：

```rust
// 当前可能的实现
impl Visitor for RouteCollector {
    fn visit_method_call(&mut self, call: &ExprMethodCall) {
        if call.method == "route" {
            self.add_route(call);
        }
    }
}
```

**缺失的功能**:
1. 未追踪 `Router::merge()` 的参数
2. 未递归解析 `Router::nest()` 的嵌套路由
3. 未解析跨模块的函数调用（`modules::router()`）

#### 测试验证

```bash
# 测试 1: 主文件生成（失败）
$ axum_doc --base-dir . --handler-file src/main.rs --output test1.json
Found 1 routes  # ❌ 遗漏了所有模块路由

# 测试 2: 单模块生成（成功但不完整）
$ axum_doc --base-dir . --handler-file src/modules/auth/handler.rs --output test2.json
Found 3 routes  # ✅ 找到了路由，但缺少 /api/v1/auth 前缀
```

### 问题 2: 路径前缀缺失

#### 根本原因

`Router::nest()` 的第一个参数是路径前缀字符串，但当前实现未提取和使用：

```rust
// axum 代码
.nest("/api/v1/auth", auth::router())
//  ^^^^^^^^^^^^ 这个前缀未被记录和应用
```

**技术分析**:

需要维护一个路径前缀栈：

```rust
struct RouteCollector {
    path_stack: Vec<String>,  // 路径前缀栈
}

impl RouteCollector {
    fn current_path(&self) -> String {
        self.path_stack.join("")
    }

    fn visit_nest(&mut self, call: &ExprMethodCall) {
        // 提取 "/api/v1/auth"
        let prefix = extract_string_literal(&call.args[0]);
        self.path_stack.push(prefix);

        // 递归解析嵌套的 router
        self.visit_expr(&call.args[1]);

        self.path_stack.pop();
    }
}
```

### 问题 3: 类型推断不准确

#### 根本原因

**源码中的类型**:
```rust
// src/modules/auth/response.rs
use uuid::Uuid;

pub struct LoginResponse {
    pub token: String,
    pub user_id: Uuid,  // ← UUID 类型
    pub username: String,
}
```

**生成的 OpenAPI**:
```json
{
  "user_id": { "type": "object" }  // ❌ 应该是 { "type": "string", "format": "uuid" }
}
```

**技术分析**:

可能的原因：
1. 未解析 `use uuid::Uuid;` 导入
2. `Uuid` 类型未在类型映射表中
3. 使用了回退策略 `OpenAPISchema::Object`

**正确的类型映射**:

| Rust 类型 | OpenAPI 类型 | OpenAPI Format |
|-----------|-------------|----------------|
| `String` | string | - |
| `i32` | integer | int32 |
| `i64` | integer | int64 |
| `u32` | integer | int32 (unsigned) |
| `u64` | integer | int64 (unsigned) |
| `f32` | number | float |
| `f64` | number | double |
| `bool` | boolean | - |
| `uuid::Uuid` | string | uuid |
| `Vec<T>` | array | - |
| `Option<T>` | T (nullable) | - |
| `chrono::DateTime` | string | date-time |

#### 测试用例

```rust
// 测试类型推断
struct TypeTest {
    string_field: String,          // string
    i32_field: i32,                // integer, int32
    i64_field: i64,                // integer, int64
    uuid_field: Uuid,              // string, uuid
    vec_field: Vec<String>,        // array<string>
    option_field: Option<String>,  // string (nullable)
}
```

### 问题 4: 文档注释未解析

#### 根本原因

**源码中的文档注释**:
```rust
/// 用户登录（返回真实 token）
///
/// POST /api/v1/auth/login
/// Body: {"username": "admin", "password": "admin123"}
async fn login(Json(form): Json<LoginForm>) -> AppResp<LoginResponse> {
    // ...
}
```

**生成的 OpenAPI**:
```json
{
  "summary": "POST login",  // ❌ 应该是 "用户登录（返回真实 token）"
  "description": null
}
```

**技术分析**:

Rust 文档注释是 `#[doc = "..."]` 属性的语法糖：

```rust
/// 用户登录
// 等价于
#[doc = "用户登录"]
async fn login() { }
```

需要提取 `#[doc]` 属性：

```rust
fn extract_docs(attrs: &[Attribute]) -> Vec<String> {
    attrs.iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            match &attr.meta {
                Meta::List(list) if list.path.is_ident("doc") => {
                    // 提取 #[doc = "content"]
                    list.tokens.to_string().trim_matches('"').to_string().ok()
                }
                _ => None
            }
        })
        .collect()
}
```

---

## 实现方案

### 方案 1: 递归路由解析器

**目标**: 支持 `Router::merge()` 和 `Router::nest()` 的递归解析

**实现**:

```rust
use syn::{Expr, ExprMethodCall, ExprCall, ItemFn};
use std::path::PathBuf;

struct RouteCollector {
    routes: Vec<Route>,
    path_stack: Vec<String>,
    file_stack: Vec<PathBuf>,
}

#[derive(Debug)]
struct Route {
    path: String,
    method: HttpMethod,
    handler: HandlerInfo,
}

#[derive(Debug)]
struct HandlerInfo {
    function_name: String,
    request_type: Option<Type>,
    response_type: Type,
    docs: Vec<String>,
}

impl RouteCollector {
    fn new() -> Self {
        Self {
            routes: Vec::new(),
            path_stack: Vec::new(),
            file_stack: Vec::new(),
        }
    }

    /// 当前完整路径（包含前缀栈）
    fn current_path(&self) -> String {
        let base = self.path_stack.join("");
        // 如果栈为空，返回根路径
        if base.is_empty() {
            "/".to_string()
        } else {
            base
        }
    }

    /// 解析 Router 表达式
    fn parse_router_expr(&mut self, expr: &Expr) {
        match expr {
            // Router::route("/path", handler)
            Expr::MethodCall(call) if call.method == "route" => {
                self.parse_route(call);
            }

            // Router::nest("/prefix", router)
            Expr::MethodCall(call) if call.method == "nest" => {
                self.parse_nest(call);
            }

            // Router::merge(router)
            Expr::MethodCall(call) if call.method == "merge" => {
                self.parse_merge(call);
            }

            // Router::new()...
            Expr::MethodCall(call) if call.method == "new" => {
                // 链式调用，继续解析接收者
                self.parse_router_expr(&call.receiver);
            }

            // 函数调用，如 modules::router()
            Expr::Call(call) => {
                self.parse_call(call);
            }

            _ => {
                // 忽略其他表达式
            }
        }
    }

    /// 解析 route() 调用
    fn parse_route(&mut self, call: &ExprMethodCall) {
        // route("/path", method_handler(handler))
        let path = self.extract_path(&call.args[0]);
        let full_path = format!("{}{}", self.current_path(), path);

        // 提取 HTTP 方法 (get, post, put, delete 等)
        let method = self.extract_http_method(&call.args[1]);

        // 提取 handler 函数
        let handler = self.extract_handler(&call.args[1]);

        self.routes.push(Route {
            path: full_path,
            method,
            handler,
        });
    }

    /// 解析 nest() 调用
    fn parse_nest(&mut self, call: &ExprMethodCall) {
        // nest("/prefix", router)
        let prefix = self.extract_path(&call.args[0]);

        // 压入前缀栈
        self.path_stack.push(prefix.clone());

        // 递归解析嵌套的 router
        self.parse_router_expr(&call.args[1]);

        // 弹出前缀栈
        self.path_stack.pop();
    }

    /// 解析 merge() 调用
    fn parse_merge(&mut self, call: &ExprMethodCall) {
        // merge(router)
        self.parse_router_expr(&call.args[0]);
    }

    /// 解析函数调用（跨模块路由）
    fn parse_call(&mut self, call: &ExprCall) {
        // modules::router()
        if let Expr::Path(func_path) = &call.func {
            let func_name = func_path.path.to_token_stream().to_string();

            // 查找被调用函数的定义文件
            if let Some(file) = self.find_function_file(&func_name) {
                self.file_stack.push(file);
                // 解析该文件中的函数
                // TODO: 加载并解析文件内容
                self.file_stack.pop();
            }
        }
    }

    /// 提取路径字符串
    fn extract_path(&self, expr: &Expr) -> String {
        match expr {
            Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => s.value(),
            _ => "/".to_string(),
        }
    }

    /// 提取 HTTP 方法
    fn extract_http_method(&self, expr: &Expr) -> HttpMethod {
        // get(handler), post(handler) 等
        match expr {
            Expr::Call(call) => {
                if let Expr::Path(func_path) = &call.func {
                    let method = func_path.path.to_token_stream().to_string();
                    match method.as_str() {
                        "get" => HttpMethod::Get,
                        "post" => HttpMethod::Post,
                        "put" => HttpMethod::Put,
                        "delete" => HttpMethod::Delete,
                        "patch" => HttpMethod::Patch,
                        _ => HttpMethod::Get,
                    }
                } else {
                    HttpMethod::Get
                }
            }
            _ => HttpMethod::Get,
        }
    }

    /// 提取 handler 函数信息
    fn extract_handler(&self, expr: &Expr) -> HandlerInfo {
        // 从方法调用中提取 handler 函数名
        // TODO: 实现完整的函数签名解析
        HandlerInfo {
            function_name: "handler".to_string(),
            request_type: None,
            response_type: Type::Verbatim(Verbatim::default()),
            docs: vec![],
        }
    }

    /// 查找函数定义的文件
    fn find_function_file(&self, func_name: &str) -> Option<PathBuf> {
        // 基于 module::function 查找文件
        // modules::auth::router -> src/modules/auth/handler.rs
        None  // TODO: 实现
    }
}

#[derive(Debug, PartialEq)]
enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}
```

### 方案 2: 增强的类型解析器

**目标**: 正确识别基础类型和常见第三方类型

**实现**:

```rust
use syn::{Type, Path, TypePath};
use std::collections::HashMap;

struct TypeResolver {
    /// 类型导入映射: local_name -> full_path
    imports: HashMap<String, String>,
    /// 类型别名: alias -> actual_type
    type_aliases: HashMap<String, Type>,
}

impl TypeResolver {
    fn new() -> Self {
        let mut imports = HashMap::new();

        // 预定义常见类型
        imports.insert("Uuid".to_string(), "uuid::Uuid".to_string());
        imports.insert("DateTime".to_string(), "chrono::DateTime".to_string());
        imports.insert("Uuid".to_string(), "uuid::Uuid".to_string());

        Self {
            imports,
            type_aliases: HashMap::new(),
        }
    }

    /// 从源文件中收集导入
    fn collect_imports(&mut self, file: &File) {
        for item in &file.items {
            if let Item::Use(use_item) = item {
                self.parse_use_item(use_item);
            }
        }
    }

    fn parse_use_item(&mut self, use_item: &ItemUse) {
        let tree = &use_item.tree;
        // 解析 use uuid::Uuid;
        // 解析 use uuid::{Uuid, UuidBuilder};
        // TODO: 实现
    }

    /// 解析类型为 OpenAPI Schema
    fn resolve_type(&self, ty: &Type) -> OpenAPISchema {
        match ty {
            // 基础类型
            Type::Path(path) if self.is_simple_type(path, "String") => {
                OpenAPISchema::String
            }

            Type::Path(path) if self.is_simple_type(path, "i32") => {
                OpenAPISchema::Integer {
                    format: Some("int32".to_string()),
                }
            }

            Type::Path(path) if self.is_simple_type(path, "i64") => {
                OpenAPISchema::Integer {
                    format: Some("int64".to_string()),
                }
            }

            Type::Path(path) if self.is_simple_type(path, "u32") => {
                OpenAPISchema::Integer {
                    format: Some("int32".to_string()),
                    minimum: Some(0.0),
                }
            }

            Type::Path(path) if self.is_simple_type(path, "u64") => {
                OpenAPISchema::Integer {
                    format: Some("int64".to_string()),
                    minimum: Some(0.0),
                }
            }

            Type::Path(path) if self.is_simple_type(path, "f32") => {
                OpenAPISchema::Number {
                    format: Some("float".to_string()),
                }
            }

            Type::Path(path) if self.is_simple_type(path, "f64") => {
                OpenAPISchema::Number {
                    format: Some("double".to_string()),
                }
            }

            Type::Path(path) if self.is_simple_type(path, "bool") => {
                OpenAPISchema::Boolean
            }

            // UUID 类型
            Type::Path(path) if self.is_uuid_type(path) => {
                OpenAPISchema::String {
                    format: Some("uuid".to_string()),
                    example: Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
                }
            }

            // DateTime 类型
            Type::Path(path) if self.is_datetime_type(path) => {
                OpenAPISchema::String {
                    format: Some("date-time".to_string()),
                    example: Some("2024-01-01T00:00:00Z".to_string()),
                }
            }

            // Vec<T>
            Type::Path(path) if self.is_vec(path) => {
                let item = self.extract_generic_arg(path, 0);
                OpenAPISchema::Array {
                    items: Box::new(self.resolve_type(&item)),
                }
            }

            // Option<T> - OpenAPI 3.0 使用 nullable
            Type::Path(path) if self.is_option(path) => {
                let inner = self.extract_generic_arg(path, 0);
                let mut schema = self.resolve_type(&inner);
                schema.set_nullable(true);
                schema
            }

            // HashMap<K, V>
            Type::Path(path) if self.is_hashmap(path) => {
                let value = self.extract_generic_arg(path, 1);
                OpenAPISchema::Map {
                    additional_properties: Box::new(self.resolve_type(&value)),
                }
            }

            // 用户自定义类型
            Type::Path(path) => {
                let type_name = self.get_type_name(path);
                OpenAPISchema::Ref {
                    reference: format!("#/components/schemas/{}", type_name),
                }
            }

            // 元组 (T1, T2, ...)
            Type::Tuple(tuple) => {
                OpenAPISchema::Array {
                    items: Box::new(OpenAPISchema::Any),
                }
            }

            // 未知类型，使用对象回退
            _ => OpenAPISchema::Object,
        }
    }

    /// 检查是否是简单类型（无泛型参数）
    fn is_simple_type(&self, path: &TypePath, name: &str) -> bool {
        path.path.segments.len() == 1
            && path.path.segments[0].ident == name
            && path.path.segments[0].arguments.is_empty()
    }

    /// 检查是否是 UUID 类型
    fn is_uuid_type(&self, path: &TypePath) -> bool {
        let name = self.get_type_name(path);
        name == "Uuid" || name.ends_with("uuid::Uuid")
    }

    /// 检查是否是 DateTime 类型
    fn is_datetime_type(&self, path: &TypePath) -> bool {
        let name = self.get_type_name(path);
        name == "DateTime" || name.ends_with("chrono::DateTime")
    }

    /// 检查是否是 Vec
    fn is_vec(&self, path: &TypePath) -> bool {
        path.path.segments.len() == 1
            && path.path.segments[0].ident == "Vec"
    }

    /// 检查是否是 Option
    fn is_option(&self, path: &TypePath) -> bool {
        path.path.segments.len() == 1
            && path.path.segments[0].ident == "Option"
    }

    /// 检查是否是 HashMap
    fn is_hashmap(&self, path: &TypePath) -> bool {
        path.path.segments.len() == 1
            && path.path.segments[0].ident == "HashMap"
    }

    /// 提取泛型参数
    fn extract_generic_arg(&self, path: &TypePath, index: usize) -> Type {
        if let Some(seg) = path.path.segments.last() {
            if let PathArguments::AngleBracketed(args) = &seg.arguments {
                if let Some(GenericArgument::Type(ty)) = args.args.get(index) {
                    return ty.clone();
                }
            }
        }
        Type::Verbatim(Verbatim::default())
    }

    /// 获取类型名称
    fn get_type_name(&self, path: &TypePath) -> String {
        path.path.to_token_stream().to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum OpenAPISchema {
    String {
        format: Option<String>,
        example: Option<String>,
    },
    Integer {
        format: Option<String>,
        minimum: Option<f64>,
    },
    Number {
        format: Option<String>,
    },
    Boolean,
    Array {
        items: Box<OpenAPISchema>,
    },
    Map {
        additional_properties: Box<OpenAPISchema>,
    },
    Object,
    Any,
    Ref {
        reference: String,
    },
}

impl OpenAPISchema {
    fn set_nullable(&mut self, nullable: bool) {
        // OpenAPI 3.0 支持 nullable
        // TODO: 实现
    }
}
```

### 方案 3: 文档注释解析

**目标**: 提取 Rust 文档注释并填充到 OpenAPI 中

**实现**:

```rust
use syn::{Attribute, ItemFn};

/// 提取文档注释
fn extract_doc_comments(attrs: &[Attribute]) -> Vec<String> {
    attrs.iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            match &attr.meta {
                Meta::List(list) if list.path.is_ident("doc") => {
                    // 提取 #[doc = "content"]
                    let content = list.tokens.to_string();
                    // 移除引号和等号
                    Some(content.trim_matches('"').trim().to_string())
                }
                Meta::Path(path) if path.is_ident("doc") => {
                    // 空的 #[doc]
                    Some(String::new())
                }
                _ => None,
            }
        })
        .collect()
}

/// 解析 Handler 函数
fn parse_handler_fn(item_fn: &ItemFn) -> HandlerInfo {
    // 提取文档注释
    let docs = extract_doc_comments(&item_fn.attrs);

    // 第一行作为 summary，其余作为 description
    let (summary, description) = if docs.is_empty() {
        (item_fn.sig.ident.to_string(), None)
    } else {
        let summary = docs[0].clone();
        let description = if docs.len() > 1 {
            Some(docs[1..].join("\n"))
        } else {
            None
        };
        (summary, description)
    };

    // 解析函数签名
    let (request_type, response_type) = parse_fn_signature(&item_fn.sig);

    HandlerInfo {
        operation_id: item_fn.sig.ident.to_string(),
        summary,
        description,
        request_type,
        response_type,
    }
}

/// 解析函数签名
fn parse_fn_signature(sig: &Signature) -> (Option<Type>, Type) {
    // async fn login(Json(form): Json<LoginForm>) -> AppResp<LoginResponse>
    let mut request_type = None;
    let mut response_type = Type::Verbatim(Verbatim::default());

    for input in &sig.inputs {
        if let FnArg::Typed(arg) = input {
            // 检查是否是 Json<T>、Query<T> 等
            if let Type::Path(path) = &arg.ty {
                if let Some(seg) = path.path.segments.first() {
                    match seg.ident.to_string().as_str() {
                        "Json" | "Query" | "Form" | "Path" => {
                            // 提取泛型参数 T
                            request_type = Some(extract_generic_arg(&path, 0));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 提取返回类型
    if let ReturnType::Type(_, ty) = &sig.output {
        response_type = (*ty).clone();
    }

    (request_type, response_type)
}
```

### 方案 4: 配置文件支持

**目标**: 允许用户通过配置文件自定义生成行为

**配置文件格式** (YAML):

```yaml
# axum-doc.yaml

# 项目配置
project:
  name: "My API"
  version: "1.0.0"
  description: "API for user management"

# 文件配置
files:
  base_dir: .
  handler_files:
    - src/main.rs
    - src/modules/*/handler.rs  # 支持通配符
  model_files:
    - src/modules/*/form.rs
    - src/modules/*/response.rs
    - src/common/response.rs

# 路径前缀配置（手动覆盖）
path_prefixes:
  src/modules/auth/handler.rs: /api/v1/auth
  src/modules/user/handler.rs: /api/v1/user
  src/modules/health/handler.rs: /api/v1/health

# 类型映射覆盖（自定义类型到 OpenAPI 的映射）
type_mappings:
  ObjectId:
    type: string
    format: object-id
  Email:
    type: string
    format: email

# 输出配置
output:
  file: openapi.json
  format: json  # 或 yaml

# 服务器配置
servers:
  - url: http://localhost:3000
    description: Development server
  - url: https://api.example.com
    description: Production server
```

**命令行使用**:

```bash
# 使用配置文件
$ axum_doc --config axum-doc.yaml

# 配置文件 + 命令行覆盖（命令行优先级更高）
$ axum_doc --config axum-doc.yaml --output openapi-new.json
```

---

## 测试用例

### 测试项目结构

创建一个完整的测试项目，包含所有场景：

```
tests/fixtures/modular_app/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── modules/
    │   ├── mod.rs
    │   ├── auth/
    │   │   ├── handler.rs
    │   │   ├── form.rs
    │   │   └── response.rs
    │   └── user/
    │       ├── handler.rs
    │       ├── form.rs
    │       └── response.rs
    └── common/
        └── response.rs
```

### 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_router() {
        // 测试 Router::merge() 解析
    }

    #[test]
    fn test_nest_router() {
        // 测试 Router::nest() 解析和前缀维护
    }

    #[test]
    fn test_nested_nest() {
        // 测试多层嵌套
        // .nest("/api", nest("/v1", nest("/auth", router())))
        // 期望路径: /api/v1/auth/login
    }

    #[test]
    fn test_cross_module_routes() {
        // 测试跨模块路由解析
    }

    #[test]
    fn test_uuid_type() {
        // 测试 UUID 类型映射
    }

    #[test]
    fn test_datetime_type() {
        // 测试 DateTime 类型映射
    }

    #[test]
    fn test_vec_type() {
        // 测试 Vec<T> 类型映射
    }

    #[test]
    fn test_option_type() {
        // 测试 Option<T> 类型映射
    }

    #[test]
    fn test_doc_comments() {
        // 测试文档注释提取
    }

    #[test]
    fn test_config_file() {
        // 测试配置文件加载
    }
}
```

### 端到端测试

```bash
#!/bin/bash
# integration-test.sh

set -e

echo "=== axum_doc Integration Tests ==="

# 测试 1: 基本路由解析
echo "Test 1: Basic route parsing"
axum_doc \
  --base-dir tests/fixtures/simple_app \
  --handler-file src/main.rs \
  --output /tmp/test1.json

# 验证输出包含预期的路由
assert_json_has_path /tmp/test1.json "/users"

# 测试 2: 模块化路由
echo "Test 2: Modular routes with merge and nest"
axum_doc \
  --base-dir tests/fixtures/modular_app \
  --handler-file src/main.rs \
  --output /tmp/test2.json

# 验证路径包含前缀
assert_json_has_path /tmp/test2.json "/api/v1/auth/login"
assert_json_has_path /tmp/test2.json "/api/v1/user/info"

# 测试 3: 类型推断
echo "Test 3: Type inference"
assert_json_schema_eq /tmp/test2.json \
  "#/components/schemas/LoginResponse/properties/user_id/type" \
  "string"

assert_json_schema_eq /tmp/test2.json \
  "#/components/schemas/LoginResponse/properties/user_id/format" \
  "uuid"

# 测试 4: 文档注释
echo "Test 4: Doc comments"
assert_json_eq /tmp/test2.json \
  "#/paths/~1api~1v1~1auth~1login/post/summary" \
  "用户登录（返回真实 token）"

# 测试 5: 配置文件
echo "Test 5: Config file"
axum_doc --config tests/fixtures/modular_app/axum-doc.yaml

echo "=== All tests passed! ==="
```

---

## 优先级建议

### P0 - 必须修复（影响可用性）

1. **文档注释解析** (1-2 天)
   - 简单且影响大
   - 提升用户体验

2. **基础类型推断** (2-3 天)
   - UUID、i64 等常见类型
   - 避免生成错误的 OpenAPI

### P1 - 重要功能（3-5 天）

3. **Router::merge() 支持** (2-3 天)
   - 递归解析 merge 参数
   - 支持跨文件追踪

4. **Router::nest() 支持** (2-3 天)
   - 维护路径前缀栈
   - 支持多层嵌套

5. **路径前缀维护** (1 天)
   - 与 nest() 支持一起实现

### P2 - 增强功能（可选）

6. **配置文件支持** (3-5 天)
   - YAML 配置解析
   - 提供更好的用户体验

7. **通配符文件匹配** (2-3 天)
   - 支持模式匹配
   - 减少命令行参数

8. **更好的错误提示** (1-2 天)
   - 详细的错误信息
   - 调试友好

---

## 附录

### A. 相关资源

- **OpenAPI 3.0 规范**: https://swagger.io/specification/
- **axum 官方文档**: https://docs.rs/axum/
- **syn crate 文档**: https://docs.rs/syn/

### B. 类似工具参考

- **utoipa**: https://github.com/juhaku/utoipa
  - 使用 derive macro 的方式
  - 对 Axum 有完整支持

- **paperclip**: https://github.com/paperclip-rs/paperclip
  - 另一个 OpenAPI 生成工具
  - 支持编译时检查

### C. 社区反馈

如果需要更多反馈，可以：
1. 在 GitHub Issues 中收集用户需求
2. 发布 beta 版本收集实际使用反馈
3. 添加用户调查问卷

---

## 总结

当前 axum_doc 在简单场景下工作良好，但对于模块化的 Axum 项目支持不足。建议优先实现 P0 和 P1 的改进，以提升工具的实用性和覆盖率。
