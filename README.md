# axum_doc

<div align="center">

[English](#english) | [中文](#chinese)

</div>

---

<div id="english">

# axum_doc

A command-line tool for automatically generating OpenAPI 3.0 JSON specifications from Axum Rust projects.

## Features
- Automatically parse axum routes and handlers, supporting nested routes
- Extract handler parameters, request bodies, response bodies, and path parameters
- Support documentation comments on handlers as OpenAPI interface descriptions
- Support multi-module, grouped output

## Installation

```sh
cargo install axum_doc
```

> Requires Rust 1.65+ and ensure `cargo` is properly configured.

## Usage

Run in your axum project root directory:

```sh
axum_doc \
  --base-dir . \
  --handler-file src/main.rs \
  --model-files src/form.rs,src/response.rs,src/types.rs \
  --output openapi.json
```

Parameter description:
- `--base-dir`: Project root directory, defaults to current directory
- `--handler-file`: Main route/handler file, defaults to `src/main.rs`
- `--model-files`: Model definition files, comma-separated, defaults to `src/form.rs,src/response.rs,src/types.rs`
- `--output`: Output OpenAPI JSON filename, defaults to `openapi-bak.json`

## Generated Output

- The generated openapi.json can be directly used with Swagger UI, Postman, Apifox, and other tools
- Supports interface grouping, parameter types, request bodies, response bodies, interface descriptions, etc.

## Common Issues
- Only supports axum 0.7 routing style
- Handlers must be standalone functions, not closures
- Only supports four extractors: `Json`, `Query`, `Path`, `Form`
- Handlers must have type signatures

## License
MIT

</div>

---

<div id="chinese">

# axum_doc

axum_doc 是一个用于从 Axum Rust 项目自动生成 OpenAPI 3.0 JSON 规范的命令行工具。

## 功能
- 自动解析 axum 路由和 handler，支持嵌套路由
- 自动提取 handler 参数、请求体、响应体、路径参数
- 支持 handler 上的文档注释作为 OpenAPI 接口描述
- 支持多模块、分组输出

## 安装

```sh
cargo install axum_doc
```

> 需要 Rust 1.65+，并确保 `cargo` 已正确配置。

## 用法

在你的 axum 项目根目录下运行：

```sh
axum_doc \
  --base-dir . \
  --handler-file src/main.rs \
  --model-files src/form.rs,src/response.rs,src/types.rs \
  --output openapi.json
```

参数说明：
- `--base-dir`：项目根目录，默认为当前目录
- `--handler-file`：主路由/handler 文件，默认为 `src/main.rs`
- `--model-files`：模型定义文件，逗号分隔，默认为 `src/form.rs,src/response.rs,src/types.rs`
- `--output`：输出的 OpenAPI JSON 文件名，默认为 `openapi-bak.json`

## 生成效果

- 生成的 openapi.json 可直接用于 Swagger UI、Postman、Apifox 等工具
- 支持接口分组、参数类型、请求体、响应体、接口描述等

## 常见问题
- 只支持 axum 0.7 路由风格
- handler 必须是独立函数，不能是闭包
- 仅支持 `Json`、`Query`、`Path`、`Form` 四种 extractor
- handler 必须有类型签名

## License
MIT

</div> 