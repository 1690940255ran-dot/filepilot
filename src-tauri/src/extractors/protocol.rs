//! 解析工作进程与界面进程之间的协议。
//!
//! ## 为什么不传路径，只传句柄
//!
//! 规格 6.2 要求「工作进程只接收批准的只读文件句柄」，并要求验证
//! 「尝试访问未授权路径」会被限制。这两件事在这里是**同一件事**：
//!
//! 界面进程用 `CreateFileW` 打开文件（`FILE_SHARE_READ`，不共享写/删），
//! 把句柄标成可继承，并且只把这**一支**句柄列进子进程的可继承清单
//! （`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`）。子进程拿到的是一个整数句柄，
//! **没有任何路径**——因此「去读一个未授权的路径」在协议层面无从表达。
//!
//! 这不是「加了检查所以安全」，而是「没有那个入口」。前者会被绕过，
//! 后者不会。负向测试正是围绕这一点写的：伪造一个带 `path` 字段的请求，
//! 工作进程必须整体拒绝，而不是忽略那个字段然后继续。
//!
//! ## 为什么整个请求用命令行参数传
//!
//! 请求只有三个短字段，用参数传就不必再铺一套 stdin 管道；
//! 少了这条通道，也就少了一处「子进程在读谁的输入」的疑问。
//! 响应走 stdout，**一行 JSON**，父进程读一行即可。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::domain::types::ExtractionStatus;

/// 协议版本。
///
/// 父进程与子进程是同一次构建的产物，理论上不会不一致；
/// 但用户完全可能把两个版本的可执行文件放在一起（覆盖安装中途、手工替换）。
/// 版本不符时子进程直接拒绝，而不是按旧格式解释新字段。
pub const PROTOCOL_VERSION: u32 = 1;

/// 这次要解析的是什么格式。
///
/// 由界面进程根据扩展名判定，**不由子进程猜**：
/// 子进程拿到的是句柄，看不到文件名，也就无从「根据扩展名推断」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceFormat {
    /// TXT / MD 等纯文本。
    Text,
    Pdf,
    Docx,
    /// PNG / JPEG：解码后交给系统 OCR（规格 6.2 第四行）。
    Image,
}

impl SourceFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceFormat::Text => "text",
            SourceFormat::Pdf => "pdf",
            SourceFormat::Docx => "docx",
            SourceFormat::Image => "image",
        }
    }

    /// 从命令行取值解析。取值区分大小写，与线上别名一致。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "text" => Some(SourceFormat::Text),
            "pdf" => Some(SourceFormat::Pdf),
            "docx" => Some(SourceFormat::Docx),
            "image" => Some(SourceFormat::Image),
            _ => None,
        }
    }
}

/// 工作进程这次要做什么。
///
/// 两种模式共用同一个可执行文件，而不是拆成两个：它们共享同一套
/// 「不碰路径、只认句柄」的约束，也共享同一份 WinRT/解析器依赖。
/// 拆开会让「OCR 探测」这条路径脱离这套约束单独演化。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkerMode {
    /// 从句柄里读一个文件并提取正文。
    Extract,
    /// 报告系统 OCR 的可用状态。
    ///
    /// 放在工作进程里做，而不是界面进程直接调 WinRT：
    /// OCR 是一种解析，界面进程不该碰解析器。更要紧的是 WinRT 的公寓模型
    /// **绑定线程**，而界面/测试进程里会有多个线程——实测多线程调用直接
    /// `STATUS_ACCESS_VIOLATION`（段错误）。
    OcrAvailability,
}

impl WorkerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkerMode::Extract => "extract",
            WorkerMode::OcrAvailability => "ocrAvailability",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "extract" => Some(WorkerMode::Extract),
            "ocrAvailability" => Some(WorkerMode::OcrAvailability),
            _ => None,
        }
    }
}

/// 工作进程的命令行参数。
///
/// **刻意没有任何路径字段。** 这不是省略，而是这个类型存在的意义：
/// 一旦有人想「顺手把路径也传过去，方便报错信息好看点」，
/// 「工作进程无法访问未授权路径」这条保证就没了。
///
/// `deny_unknown_fields` 让「多传了一个字段」变成一次**明确拒绝**：
/// 未知字段意味着要么调用方与子进程版本不一致，要么有人正在试探协议。
/// 两种情况下按自己的理解继续跑都是错的。
///
/// 提取相关的三个字段是可选的，由 [`WorkerArgs::validate`] 按模式补上要求——
/// OCR 探测模式下它们本来就不该出现，硬要求反而会逼调用方填假值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerArgs {
    pub version: u32,
    pub mode: WorkerMode,
    /// 从父进程继承来的**只读**文件句柄值（仅提取模式）。
    ///
    /// 句柄值只在子进程自己的句柄表里有意义，因此它不构成一种能力外泄：
    /// 别人拿到这个数字，在自己的进程里什么也打不开。
    pub handle: Option<u64>,
    /// 用于把结果对上是哪一条文件记录（仅提取模式）。
    pub file_id: Option<String>,
    /// 要按哪种格式解析（仅提取模式）。
    pub format: Option<SourceFormat>,
}

impl WorkerArgs {
    /// 从 `std::env::args()` 之后的部分解析。
    ///
    /// 手写而不是引 clap：参数只有几个、没有子命令、也不需要帮助文本。
    /// 更重要的是**未知参数要报错而不是忽略**——忽略会让
    /// 「你以为传了其实没传」变成一个安静的失败。
    pub fn parse<I: IntoIterator<Item = String>>(argv: I) -> Result<Self, String> {
        let mut version = None;
        let mut mode = None;
        let mut handle = None;
        let mut file_id = None;
        let mut format = None;

        let mut items = argv.into_iter();
        while let Some(flag) = items.next() {
            let value = items
                .next()
                .ok_or_else(|| format!("参数 {flag} 缺少取值"))?;
            match flag.as_str() {
                "--version" => version = Some(value.parse::<u32>().map_err(|_| "版本号不是整数")?),
                "--mode" => {
                    mode = Some(WorkerMode::parse(&value).ok_or_else(|| {
                        format!("未知模式 {value:?}（只支持 extract/ocrAvailability）")
                    })?)
                }
                "--handle" => handle = Some(value.parse::<u64>().map_err(|_| "句柄不是整数")?),
                "--file-id" => file_id = Some(value),
                "--format" => {
                    format = Some(SourceFormat::parse(&value).ok_or_else(|| {
                        format!("未知格式 {value:?}（只支持 text/pdf/docx/image）")
                    })?)
                }
                other => return Err(format!("不认识的参数 {other:?}")),
            }
        }

        Ok(Self {
            version: version.ok_or("缺少 --version")?,
            mode: mode.ok_or("缺少 --mode")?,
            handle,
            file_id,
            format,
        })
    }

    /// 按模式检查必填项。
    ///
    /// 提取模式缺任何一个都是**用法错误**，必须当场拒绝：
    /// 少了句柄它什么都读不到，少了格式它不知道该按哪种方式解——
    /// 继续跑只会产出一个含混的失败，让父进程以为是文件的问题。
    pub fn validate(&self) -> Result<(), String> {
        if self.mode != WorkerMode::Extract {
            return Ok(());
        }
        if self.handle.is_none() {
            return Err("提取模式缺少 --handle".to_owned());
        }
        if self.file_id.is_none() {
            return Err("提取模式缺少 --file-id".to_owned());
        }
        if self.format.is_none() {
            return Err("提取模式缺少 --format".to_owned());
        }
        Ok(())
    }

    /// 提取模式下的三个字段。调用方**必须先 `validate()`**。
    pub fn extraction(&self) -> Option<(u64, &str, SourceFormat)> {
        Some((self.handle?, self.file_id.as_deref()?, self.format?))
    }
}

/// 工作进程的产出。
///
/// 与 `domain::types::Extraction` 的形状一致，但**少一个字段**：
/// `sourceFingerprint`。指纹由界面进程算——它才是持有卷身份与批准根的那一方，
/// 而子进程看到的只有一个句柄。让子进程去算指纹会把「谁负责核对稳定性」
/// 这件事故意搅浑。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerOutcome {
    pub version: u32,
    pub file_id: String,
    pub status: ExtractionStatus,
    /// 失败或降级的原因码。成功时为 `None`。
    pub code: Option<String>,
    /// 提取到的正文。规格 6.2：只留在会话内存，不写持久日志。
    pub text: String,
    pub truncated: bool,
    pub evidence: Vec<crate::domain::types::Evidence>,
}

impl WorkerOutcome {
    /// 一个明确的失败结果。
    pub fn failed(file_id: &str, code: &str) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            file_id: file_id.to_owned(),
            status: ExtractionStatus::Failed,
            code: Some(code.to_owned()),
            text: String::new(),
            truncated: false,
            evidence: Vec::new(),
        }
    }

    /// 一个「这个格式不支持提内容」的结果。
    ///
    /// 与 `failed` 分开是有意的：`unsupported` 说的是「这类文件本来就没有
    /// 可提取的正文」（例如扫描型 PDF 只有图），界面据此显示的是
    /// 「只用文件名和元信息」；而 `failed` 说的是「本该能读却没读成」，
    /// 用户应当看到原因并可能去处理那个文件。把两者混为一谈，
    /// 会让一个坏文件看起来像一类正常文件。
    pub fn unsupported(file_id: &str, code: &str) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            file_id: file_id.to_owned(),
            status: ExtractionStatus::Unsupported,
            code: Some(code.to_owned()),
            text: String::new(),
            truncated: false,
            evidence: Vec::new(),
        }
    }
}

/// 工作进程用来报「我起不来/参数不对」的退出码。
///
/// 与「解析失败」分开：后者是一份正常的 `WorkerOutcome`（退出码 0），
/// 前者意味着父进程连一次像样的对话都没得到，应当按环境问题处理。
pub const WORKER_USAGE_EXIT_CODE: i32 = 64;

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_well_formed_argument_list_parses() {
        let args = WorkerArgs::parse(argv(&[
            "--version",
            "1",
            "--mode",
            "extract",
            "--handle",
            "4242",
            "--file-id",
            "file-1",
            "--format",
            "pdf",
        ]))
        .expect("应当解析成功");

        assert_eq!(args.version, PROTOCOL_VERSION);
        assert_eq!(args.mode, WorkerMode::Extract);
        assert_eq!(args.handle, Some(4242));
        assert_eq!(args.file_id.as_deref(), Some("file-1"));
        assert_eq!(args.format, Some(SourceFormat::Pdf));
        args.validate().expect("提取模式的三个字段都在");
    }

    #[test]
    fn a_path_argument_is_rejected_rather_than_ignored() {
        // 这是本协议最重要的一条负向断言。
        // 「顺手支持一下 --path」会让「工作进程无法访问未授权路径」
        // 这条保证消失，而这正是规格 6.2 点名要验证的东西。
        let error = WorkerArgs::parse(argv(&[
            "--version",
            "1",
            "--handle",
            "1",
            "--file-id",
            "f",
            "--format",
            "text",
            "--path",
            r"C:\Windows\System32\config\SAM",
        ]))
        .expect_err("带路径的调用必须被拒绝");

        assert!(
            error.contains("--path"),
            "错误里要点名是哪个参数不认识，否则调用方只能猜：{error}"
        );
    }

    #[test]
    fn an_unknown_format_is_rejected() {
        let error = WorkerArgs::parse(argv(&[
            "--version",
            "1",
            "--mode",
            "extract",
            "--handle",
            "1",
            "--file-id",
            "f",
            "--format",
            "exe",
        ]))
        .expect_err("未知格式必须被拒绝");
        assert!(error.contains("exe"));
    }

    #[test]
    fn a_flag_without_a_value_is_reported_as_such() {
        let error = WorkerArgs::parse(argv(&["--version"])).expect_err("缺取值应当报错");
        assert!(error.contains("缺少取值"), "{error}");
    }

    #[test]
    fn a_missing_required_flag_is_reported() {
        let error = WorkerArgs::parse(argv(&["--version", "1"])).expect_err("缺参数应当报错");
        assert!(error.contains("--mode"), "{error}");
    }

    #[test]
    fn the_wire_shape_carries_no_path_like_field() {
        // 结构层面的兜底：序列化出来的字段名里不该出现任何像路径的东西。
        // 这条断言在有人给 WorkerArgs 加字段时会被提醒到——
        // 而它失败的样子（多了一个字段名）比「安全保证悄悄消失」好得多。
        let encoded = serde_json::to_string(&WorkerArgs {
            version: 1,
            mode: WorkerMode::Extract,
            handle: Some(7),
            file_id: Some("f".to_owned()),
            format: Some(SourceFormat::Text),
        })
        .expect("应能序列化");

        for forbidden in ["path", "root", "dir", "url"] {
            assert!(
                !encoded.to_lowercase().contains(forbidden),
                "协议里不该出现 {forbidden:?}：{encoded}"
            );
        }
    }

    #[test]
    fn the_ocr_probe_mode_needs_no_handle_or_format() {
        // 探测模式不读文件，因此不该被逼着填假句柄。
        // 硬要求会让调用方传一个 `--handle 0` 进来——那是一个
        // 看起来像句柄、实际什么也不是的数字，比缺字段更难查。
        let args = WorkerArgs::parse(argv(&["--version", "1", "--mode", "ocrAvailability"]))
            .expect("探测模式应当解析成功");

        assert_eq!(args.mode, WorkerMode::OcrAvailability);
        assert!(args.handle.is_none());
        args.validate().expect("探测模式没有必填项");
        assert!(args.extraction().is_none());
    }

    #[test]
    fn extract_mode_without_a_handle_is_a_usage_error() {
        // 少了句柄它什么都读不到。继续跑只会产出一个含混的失败，
        // 让父进程以为是文件的问题——所以必须当场拒绝。
        let args = WorkerArgs::parse(argv(&[
            "--version",
            "1",
            "--mode",
            "extract",
            "--file-id",
            "f",
            "--format",
            "text",
        ]))
        .expect("解析本身应当成功");

        let error = args.validate().expect_err("缺少句柄必须报用法错误");
        assert!(error.contains("--handle"), "{error}");
    }

    #[test]
    fn an_unknown_mode_is_rejected() {
        let error = WorkerArgs::parse(argv(&["--version", "1", "--mode", "whatever"]))
            .expect_err("未知模式必须拒绝");
        assert!(error.contains("whatever"), "{error}");
    }

    #[test]
    fn unknown_fields_in_a_payload_are_rejected() {
        // 子进程按 JSON 解析时同样要拒绝未知字段——有人直接构造一份
        // 带 path 的 JSON 塞进来，不能因为「字段没用上」就放过。
        let forged = r#"{"version":1,"handle":1,"fileId":"f","format":"text","path":"C:\\x"}"#;
        let error =
            serde_json::from_str::<WorkerArgs>(forged).expect_err("带未知字段的载荷必须被拒绝");
        assert!(error.to_string().contains("path"), "{error}");
    }
}

/// 工作进程报回来的 OCR 可用状态。
///
/// 与 `platform::ocr::OcrAvailability` 一一对应，但**独立成一个可序列化的类型**：
/// 后者带着 WinRT 那侧的语义，不该让 IPC 契约依赖它；而且工作进程与界面进程
/// 之间走的本来就只有纯数据。
///
/// 它同时是**设置页要显示的东西**（T11：设置页显示 OCR 可用状态和原因），
/// 所以进了 IPC 契约，需要和别的契约类型一样可生成 JSON Schema 与 TS 类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct OcrAvailabilityReport {
    /// 用哪个变体：`available` / `noRecognizerLanguage` / `noChineseLanguage` / `unsupported`。
    pub status: String,
    /// 可识别的语言标签。不可用时为空或列出已装但不含中文的语言。
    pub languages: Vec<String>,
    /// 给用户看的一句话，**必须包含下一步动作**。
    pub message: String,
}

impl OcrAvailabilityReport {
    pub fn is_usable(&self) -> bool {
        self.status == "available"
    }
}
