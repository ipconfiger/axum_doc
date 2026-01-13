# axum_doc

<div align="center">

[English](#english) | [中文](#chinese)

</div>

---

<div id="english">

# axum_doc

A command-line tool for automatically generating OpenAPI 3.0 JSON specifications from Axum Rust projects through static code analysis.

## ✨ Features

### v0.2.0 Highlights
- **Enhanced Type Mapping** - Support for UUID, DateTime, Duration, HashMap, usize/isize
- **Nullable Support** - Proper `Option<T>` handling with OpenAPI `nullable` field
- **Array Support** - Complete `Vec<T>` schema generation with item types
- **Router::merge()** - Support for cross-module route composition
- **Nested Modules** - Enhanced `Router::nest()` with deep nesting support
- **Doc Comments** - Automatic extraction of summary and description from `///` comments
- **Clean Output** - Trimmed doc comments for professional OpenAPI specs

### Core Features
- Automatically parse Axum routes and handlers
- Support for nested routes with path prefix tracking
- Extract handler parameters, request bodies, response bodies, and path parameters
- Generate type-safe OpenAPI schemas from Rust structs
- Support for modular router organization

## 📦 Installation

```sh
cargo install axum_doc
```

> Requires Rust 1.65+ and ensure `cargo` is properly configured.

## 🚀 Usage

Run in your Axum project root directory:

```sh
axum_doc \
  --base-dir . \
  --handler-file src/main.rs \
  --model-files src/form.rs,src/response.rs,src/types.rs \
  --output openapi.json
```

### Parameters

- `--base-dir`: Project root directory (default: current directory)
- `--handler-file`: Main route/handler file (default: `src/main.rs`)
- `--model-files`: Model definition files, comma-separated (default: `src/form.rs,src/response.rs,src/types.rs`)
- `--output`: Output OpenAPI JSON filename (default: `openapi-bak.json`)

## 📖 Example

Given the following Axum code:

```rust
use axum::{Json, routing::post, Router};
use serde::{Deserialize, Serialize};

/// User login credentials
#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// User login response with token
#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: uuid::Uuid,
    pub username: String,
}

/// User login endpoint
///
/// Authenticates a user and returns a JWT token.
async fn login(Json(form): Json<LoginForm>) -> Json<LoginResponse> {
    Json(LoginResponse {
        token: "jwt_token".to_string(),
        user_id: uuid::Uuid::new_v4(),
        username: form.username,
    })
}

fn app() -> Router {
    Router::new().route("/login", post(login))
}
```

Run `axum_doc` to generate:

```json
{
  "openapi": "3.0.0",
  "info": {
    "title": "Generated API",
    "version": "1.0.0"
  },
  "paths": {
    "/login": {
      "post": {
        "summary": "User login endpoint",
        "description": "Authenticates a user and returns a JWT token.",
        "operationId": "login",
        "requestBody": {
          "content": {
            "application/json": {
              "schema": { "$ref": "#/components/schemas/LoginForm" }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Successful response",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/LoginResponse" }
              }
            }
          }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "LoginResponse": {
        "type": "object",
        "properties": {
          "token": { "type": "string" },
          "user_id": {
            "type": "string",
            "format": "uuid"
          },
          "username": { "type": "string" }
        }
      }
    }
  }
}
```

## 🔧 Supported Types

### Primitive Types
| Rust Type | OpenAPI Type | Format |
|-----------|--------------|--------|
| `String`, `&str` | string | - |
| `i32`, `u32` | integer | int32 |
| `i64`, `u64`, `usize`, `isize` | integer | int64 |
| `f32` | number | float |
| `f64` | number | double |
| `bool` | boolean | - |

### Special Types
| Rust Type | OpenAPI Type | Format | Example |
|-----------|--------------|--------|---------|
| `uuid::Uuid` | string | uuid | `550e8400-e29b-41d4-a716-446655440000` |
| `chrono::DateTime` | string | date-time | `2024-01-01T00:00:00Z` |
| `std::time::Duration` | string | duration | - |

### Generic Types
| Rust Type | OpenAPI Type | Notes |
|-----------|--------------|-------|
| `Vec<T>` | array | Items schema properly resolved |
| `Option<T>` | T | With `nullable: true` |
| `HashMap<K,V>` | object | With `additionalProperties` |

## 🏗️ Router Organization

### Nested Routes

```rust
Router::new()
    .nest("/api/v1", user::router())  // Path prefix automatically applied
```

**⚠️ Anti-Pattern to Avoid:**

Don't nest the same path prefix multiple times:

```rust
// In modules/mod.rs:
Router::new().nest("/api/v1/user", user::router())

// In modules/user/mod.rs (WRONG - causes /api/v1/user/api/v1/user/login):
Router::new().nest("/api/v1/user", handler::router())

// Correct approach - just return the handler router:
pub fn router() -> Router {
    handler::router()  // No double-nesting
}
```

### Merged Routes

```rust
Router::new()
    .route("/", get(root))
    .merge(auth::router())  // Cross-module composition
```

## 📝 Documentation Comments

Use `///` doc comments to document your endpoints:

```rust
/// Get user by ID
///
/// Retrieves user information by their unique identifier.
/// Returns 404 if the user doesn't exist.
async fn get_user(Path(id): Path<Uuid>) -> Json<User> {
    // ...
}
```

- First line → `summary`
- Remaining lines → `description`
- Blank lines are automatically filtered

## ⚠️ Current Limitations

- Only supports Axum 0.7 routing style
- Handlers must be standalone functions, not closures
- Supported extractors: `Json`, `Query`, `Path`, `Form`
- Handlers must have explicit type signatures
- Be careful with path prefix duplication: avoid double-nesting the same path (e.g., `.nest("/api/v1", module_router())` in both parent and child modules)

## 🔄 Changelog

### v0.2.1 (Latest)
- 🐛 Fixed module path resolution for nested directory structures
- ✨ Added `current_module` tracking to distinguish sibling vs nested modules
- ✨ Added `calculate_module_path()` helper for accurate module path computation
- ✨ Added `extract_module_from_path()` helper to derive module context from file paths
- ✨ Improved module file discovery in complex project structures
- ✅ Properly handles nested modules (e.g., `modules/auth/handler.rs`)
- 📝 Added detailed test analysis documentation
- 🔧 Removed unused `module_stack` field (replaced by `current_module`)

### v0.2.0
- ✨ Added UUID, DateTime, Duration type support
- ✨ Added usize/isize type support
- ✨ Fixed Option<T> to use `nullable: true` instead of `"object"`
- ✨ Fixed Vec<T> to properly resolve items schema
- ✨ Added HashMap<K,V> support with `additionalProperties`
- ✨ Added Router::merge() support for cross-module routes
- ✨ Enhanced Router::nest() with nested module support
- ✨ Improved doc comment extraction with automatic trimming
- ✨ Filter empty lines in doc comments
- ✨ Better error messages and file not found warnings
- ✅ Added 22 unit tests and 15 integration tests
- 🐛 Fixed various type mapping issues

### v0.1.1
- Initial release with basic route parsing

## 🧪 Testing

The project includes comprehensive tests:

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --bin axum_doc

# Run only integration tests
cargo test --test integration_test
```

## 📄 License

MIT

---

</div>

---

<div id="chinese">

# axum_doc

axum_doc 是一个用于从 Axum Rust 项目自动生成 OpenAPI 3.0 JSON 规范的命令行工具，通过静态代码分析实现。

## ✨ 功能特性

### v0.2.0 亮点
- **增强的类型映射** - 支持 UUID、DateTime、Duration、HashMap、usize/isize
- **可空类型支持** - 正确处理 `Option<T>`，生成 OpenAPI `nullable` 字段
- **数组支持** - 完整的 `Vec<T>` schema 生成，包含元素类型
- **Router::merge() 支持** - 支持跨模块路由组合
- **嵌套模块** - 增强的 `Router::nest()` 支持，支持深层嵌套
- **文档注释** - 自动从 `///` 注释提取摘要和描述
- **清洁输出** - 自动修剪文档注释，生成专业的 OpenAPI 规范

### 核心功能
- 自动解析 Axum 路由和处理器
- 支持嵌套路由，自动跟踪路径前缀
- 提取处理器参数、请求体、响应体和路径参数
- 从 Rust 结构体生成类型安全的 OpenAPI schema
- 支持模块化路由组织

## 📦 安装

```sh
cargo install axum_doc
```

> 需要 Rust 1.65+，并确保 `cargo` 已正确配置。

## 🚀 使用方法

在你的 Axum 项目根目录下运行：

```sh
axum_doc \
  --base-dir . \
  --handler-file src/main.rs \
  --model-files src/form.rs,src/response.rs,src/types.rs \
  --output openapi.json
```

### 参数说明

- `--base-dir`：项目根目录（默认：当前目录）
- `--handler-file`：主路由/处理器文件（默认：`src/main.rs`）
- `--model-files`：模型定义文件，逗号分隔（默认：`src/form.rs,src/response.rs,src/types.rs`）
- `--output`：输出的 OpenAPI JSON 文件名（默认：`openapi-bak.json`）

## 📖 使用示例

给定以下 Axum 代码：

```rust
use axum::{Json, routing::post, Router};
use serde::{Deserialize, Serialize};

/// 用户登录凭据
#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// 用户登录响应
#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: uuid::Uuid,
    pub username: String,
}

/// 用户登录端点
///
/// 验证用户身份并返回 JWT token。
async fn login(Json(form): Json<LoginForm>) -> Json<LoginResponse> {
    Json(LoginResponse {
        token: "jwt_token".to_string(),
        user_id: uuid::Uuid::new_v4(),
        username: form.username,
    })
}

fn app() -> Router {
    Router::new().route("/login", post(login))
}
```

运行 `axum_doc` 生成：

```json
{
  "openapi": "3.0.0",
  "info": {
    "title": "Generated API",
    "version": "1.0.0"
  },
  "paths": {
    "/login": {
      "post": {
        "summary": "用户登录端点",
        "description": "验证用户身份并返回 JWT token。",
        "operationId": "login",
        "requestBody": {
          "content": {
            "application/json": {
              "schema": { "$ref": "#/components/schemas/LoginForm" }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Successful response",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/LoginResponse" }
              }
            }
          }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "LoginResponse": {
        "type": "object",
        "properties": {
          "token": { "type": "string" },
          "user_id": {
            "type": "string",
            "format": "uuid"
          },
          "username": { "type": "string" }
        }
      }
    }
  }
}
```

## 🔧 支持的类型

### 基本类型
| Rust 类型 | OpenAPI 类型 | 格式 |
|-----------|-------------|------|
| `String`, `&str` | string | - |
| `i32`, `u32` | integer | int32 |
| `i64`, `u64`, `usize`, `isize` | integer | int64 |
| `f32` | number | float |
| `f64` | number | double |
| `bool` | boolean | - |

### 特殊类型
| Rust 类型 | OpenAPI 类型 | 格式 | 示例 |
|-----------|-------------|------|-----|
| `uuid::Uuid` | string | uuid | `550e8400-e29b-41d4-a716-446655440000` |
| `chrono::DateTime` | string | date-time | `2024-01-01T00:00:00Z` |
| `std::time::Duration` | string | duration | - |

### 泛型类型
| Rust 类型 | OpenAPI 类型 | 说明 |
|-----------|-------------|------|
| `Vec<T>` | array | 正确解析元素类型 |
| `Option<T>` | T | 添加 `nullable: true` |
| `HashMap<K,V>` | object | 包含 `additionalProperties` |

## 🏗️ 路由组织

### 嵌套路由

```rust
Router::new()
    .nest("/api/v1", user::router())  // 路径前缀自动应用
```

**⚠️ 避免的反模式：**

不要多次嵌套相同的路径前缀：

```rust
// 在 modules/mod.rs 中：
Router::new().nest("/api/v1/user", user::router())

// 在 modules/user/mod.rs 中（错误 - 会导致 /api/v1/user/api/v1/user/login）：
Router::new().nest("/api/v1/user", handler::router())

// 正确的方法 - 直接返回 handler 的 router：
pub fn router() -> Router {
    handler::router()  // 避免双重嵌套
}
```

### 合并路由

```rust
Router::new()
    .route("/", get(root))
    .merge(auth::router())  // 跨模块组合
```

## 📝 文档注释

使用 `///` 文档注释来记录端点：

```rust
/// 根据 ID 获取用户
///
/// 通过唯一标识符检索用户信息。
/// 如果用户不存在，返回 404。
async fn get_user(Path(id): Path<Uuid>) -> Json<User> {
    // ...
}
```

- 第一行 → `summary`
- 剩余行 → `description`
- 空行自动过滤

## ⚠️ 当前限制

- 只支持 Axum 0.7 路由风格
- handler 必须是独立函数，不能是闭包
- 支持的提取器：`Json`、`Query`、`Path`、`Form`
- handler 必须有显式类型签名
- 注意路径前缀重复问题：避免在父模块和子模块中双重嵌套相同路径（例如，父模块和子模块中都使用 `.nest("/api/v1", module_router())`）

## 🔄 更新日志

### v0.2.1 (最新版本)
- 🐛 修复嵌套目录结构中的模块路径解析
- ✨ 新增 `current_module` 跟踪以区分兄弟模块和嵌套模块
- ✨ 新增 `calculate_module_path()` 辅助函数用于精确计算模块路径
- ✨ 新增 `extract_module_from_path()` 辅助函数从文件路径推导模块上下文
- ✨ 改进复杂项目结构中的模块文件发现
- ✅ 正确处理嵌套模块（如 `modules/auth/handler.rs`）
- 📝 新增详细的测试分析文档
- 🔧 移除未使用的 `module_stack` 字段（由 `current_module` 替代）

### v0.2.0
- ✨ 新增 UUID、DateTime、Duration 类型支持
- ✨ 新增 usize/isize 类型支持
- ✨ 修复 Option<T> 使用 `nullable: true` 而非 `"object"`
- ✨ 修复 Vec<T> 正确解析 items schema
- ✨ 新增 HashMap<K,V> 支持，包含 `additionalProperties`
- ✨ 新增 Router::merge() 支持跨模块路由
- ✨ 增强 Router::nest() 支持嵌套模块
- ✨ 改进文档注释提取，自动修剪空格
- ✨ 过滤文档注释中的空行
- ✨ 改进错误提示和文件未找到警告
- ✨ 新增 22 个单元测试和 15 个集成测试
- 🐛 修复多个类型映射问题

### v0.1.1
- 初始版本，支持基本路由解析

## 🧪 测试

项目包含全面的测试：

```bash
# 运行所有测试
cargo test

# 仅运行单元测试
cargo test --bin axum_doc

# 仅运行集成测试
cargo test --test integration_test
```

## 📄 许可证

MIT

---

</div>
