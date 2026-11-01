//! 管理界面、发信接口和投递队列。
//!
//! `core` 放配置、加密、错误、模型和进程状态。`http` 放界面与接口。
//! `mail` 放域名检查、预览、队列和投递。`ops` 放 Cloudflare、初始模板和版本检查。
//! 根路径保持 `bunmail_rs::send` 这种稳定入口，二进制和集成测试不用跟着目录走。

mod core;
mod http;
mod mail;
mod ops;

pub use core::{config, crypto, error, model, state};
pub use http::{api, auth, contract as http_api, router as web};
pub use mail::{dns, preview, queue, send, smtp};
pub use ops::{cloudflare, templates as initial_templates, updates};
