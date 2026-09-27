/**
 * 简体中文文案。规格 2.3：界面语言默认简体中文，**文案集中管理**。
 * 组件里不允许出现硬编码的用户可见字符串。
 */
export const zhCN = {
  app: {
    name: '文件领航',
    nameEn: 'FilePilot',
    tagline: '用自然语言整理本地文件，每一步都可预览、可确认、可撤销',
    versionLabel: '版本',
    workspace: '本地文件工作台',
    safety: '先预览，再确认',
    safetyHint: '每一次整理都有记录可查。',
  },

  nav: {
    home: '首页',
    preview: '预览',
    history: '历史',
    settings: '设置',
    ariaLabel: '主导航',
  },

  common: {
    loading: '加载中…',
    retry: '重试',
    unknownError: '发生未知错误。',
    notAvailableInBrowser: '当前不在桌面应用环境中运行。',
  },

  home: {
    heading: '整理一个文件夹',
    description:
      '选择一个本地文件夹，扫描后输入整理要求，生成建议并逐条确认。所有变更在确认前不会写入磁盘。',
    chooseRoot: '选择文件夹',
    changeRoot: '换一个文件夹',
    rootChosen: '已选择',
    rootNotChosen: '尚未选择文件夹',
    startScan: '开始扫描',
    scanning: '正在扫描…',
    cancelScan: '取消扫描',
    scanCancelled: '扫描已取消。',
    rootSelectionCancelled: '未选择文件夹。',
    rescan: '重新扫描',
    // 规格 6.1：达到上限时必须要求用户缩小范围，不能说「扫描完成」
    truncatedWarning:
      '扫描达到上限并已截断，结果并不完整。请选择更小的文件夹或减少文件数量后重试。',
    summaryTitle: '扫描结果',
    summaryTotal: '条目总数',
    summaryUsable: '可整理文件',
    summarySkipped: '已跳过',
    skippedHint: '被跳过的项会在文件列表中给出原因（隐藏、链接、临时下载文件等）。',
    noFilesHint: '这个文件夹里没有可整理的普通文件。请换一个文件夹。',
    modeHeading: '当前整理模式',
    modeRules: '规则模式（按类型 / 按修改月份，完全离线可用）',
    modeAiLocal: 'AI 本地模式',
    modeAiCloud: 'AI 云端模式',
    buildPlan: '生成整理建议',
    building: '正在生成…',
    buildFailed: '生成整理建议失败。',
    buildPlanNotice: '生成建议只做只读分析，不会移动、重命名或删除任何文件。',
    ruleHeading: '选择整理方式',
    byType: '按文件类型',
    byTypeHint: '文档、图片、音视频，各归其位',
    byMonth: '按修改月份',
    byMonthHint: '按最后修改时间归入年月文件夹',
    steps: ['选择文件夹', '扫描与设置', '预览并确认'],
  },

  history: {
    heading: '整理历史',
    empty: '还没有任何整理记录。完成一次整理后，计划、执行日志与恢复状态会显示在这里。',
    emptyHint: '历史记录保存在本机数据库中，不会上传。',
    reload: '刷新',
    loadMore: '加载更早记录',
    loadingMore: '正在加载…',
    startedAt: '开始时间',
    status: '状态',
    appliedColumn: '已移动',
    failedColumn: '失败',
    skippedColumn: '未执行',
    detail: '查看详情',
    collapse: '收起',
    // 恢复入口：只给「重新预览」，不给「再执行一次」
    reproviewButton: '重新预览',
    reproviewHint:
      '这份计划已经执行过，不能直接再跑一次——重复执行会把已经移动过的文件再挪一遍。请重新扫描并生成一份新计划。',
    recoveryHint:
      '这次执行留下了无法自动判定的项，需要先在恢复流程里核对。在核对完成之前，应用不接受新的整理任务。',
    recoveryPending: '这一项需要人工核对后才能继续。',
    recoveryOpen: '去核对',
    statusCompleted: '全部完成',
    statusPartial: '部分完成',
    statusFailed: '未能执行',
    statusCancelled: '已停止',
    statusRecoveryRequired: '需要人工核对',
    statusRunning: '进行中',
    statusQueued: '排队中',

    // PR-003：逐文件明细
    //
    // 在加这一组之前，明细区只渲染 `IssueList`，而一次顺利的整理 issues 为空，
    // 于是展开后只有「没有发现问题。」——整理两次以上就分不清哪条是哪次。
    itemDetailHeading: '这次具体动了哪些文件',
    itemDetailLoading: '正在读取明细…',
    itemDetailEmpty: '这次执行没有产生任何文件操作记录。',
    itemDetailLoadFailed: '无法读取这次整理的明细。',
    itemRetry: '重试',
    itemColumnSource: '原位置',
    itemColumnTarget: '整理后',
    itemColumnStatus: '结果',
    itemStatusSummary: (total: number, failed: number) =>
      failed === 0 ? `共 ${total} 项，全部正常` : `共 ${total} 项，其中 ${failed} 项未完成`,
    /*
      计划标识。列表行上显示它的前 8 位，用来区分「哪条历史对应哪次整理」。
      同一次生成的计划被执行、被撤销，标识相同；两次不同的整理，标识必然不同。
    */
    planIdLabel: '计划',
  },

  settings: {
    heading: '设置',
    modeLabel: '整理模式',
    scanMaxFilesLabel: '单次扫描文件数上限',
    scanMaxDepthLabel: '递归深度上限',
    providerLabel: '已选模型提供商',
    providerNone: '未配置',
    readOnlyNotice:
      '当前整理模式和扫描上限为只读；模型提供商可以在下方管理。',
    secretsNotice:
      'API Key 只保存到 Windows 凭据存储，界面保存后不再读回明文。设置表内不存任何密钥。',

    // 图片文字识别（规格 6.2 第四行）
    ocrLabel: '图片文字识别（OCR）',
    /**
     * 这三句是**兜底**：正常情况下显示的是后端给的原因。
     * 只有连查询都没走通（IPC 失败）时才用得到。
     */
    ocrChecking: '正在检查本机的识别语言…',
    ocrAvailable: '可用。可识别的语言：',
    ocrUnreadable: '无法读取 OCR 状态。这不影响规则模式，也不影响文字类文件的提取。',
    /**
     * 规格 T11：用户无需 OCR 也可用规则模式。
     * 这句话必须在界面上，而不能只写在文档里——用户看到「不可用」
     * 时第一反应是「那这软件还能用吗」。
     */
    ocrFallbackNotice:
      '没有 OCR 也能正常使用：图片只按文件名和时间整理，文字类文件的正文照常提取，规则模式完全可用。',

    // 模型提供商（规格 5.2 / 6.4）
    providerHeading: '模型提供商',
    providerDescription:
      '用于「AI 建议」模式。不配置也能用规则模式整理文件——规则模式不联网、也不发送任何内容。',
    providerEmpty: '还没有配置任何提供商。',
    providerListLabel: '已配置的提供商',
    providerIdLabel: '标识',
    providerKindLabel: '类型',
    providerKindLocal: '本机（Ollama）',
    providerKindCompatible: '兼容云（OpenAI 格式）',
    providerEndpointLabel: '服务地址',
    providerModelLabel: '模型名',
    providerSecretLabel: 'API Key',
    providerSecretPlaceholder: '留空表示不改动已保存的密钥',
    providerSecretHint: '密钥保存后不再显示。要更换就填一个新的。',
    providerSecretSaved: '已保存密钥',
    providerSecretAbsent: '未配置密钥',
    providerLocalHint: '本机地址只允许回环（127.0.0.1 或 ::1），不接受域名。',
    providerCloudHint: '云端地址必须使用 https://，且不会跟随重定向。',
    providerSave: '保存',
    providerSaving: '正在保存…',
    providerSaved: '已保存。密钥只在系统凭据存储里，设置表里不存密钥。',
    providerEdit: '编辑',
    providerTest: '测试连通性',
    providerTesting: '正在测试…',
  },

  ai: {
    heading: 'AI 整理',
    modeRules: '规则（不联网）',
    modeAiLocal: '本机模型（内容不出这台电脑）',
    modeAiCloud: '云端模型（内容会发送到提供商）',
    modeHeading: '处理方式',
    instructionLabel: '整理要求',
    instructionHint: '例如「按项目分类」「把发票和合同分开」。最多 1,000 字。',
    instructionPlaceholder: '用一句话说明你想怎么整理',
    previewAction: '查看将要发送的内容',
    preparing: '正在准备待发送内容…',
    reviewHeading: '将要发送给模型的内容',
    reviewTableCaption: '待发送文件清单',
    reviewDescription:
      '下面是本次**实际**会发出去的字段。请确认没有不该外发的内容，再决定是否继续。这一步只在本地进行，还没有发出任何请求。',
    reviewFiles: '文件',
    reviewCharacters: '字符',
    reviewModel: '模型',
    reviewInstruction: '整理要求',
    columnFile: '文件名',
    columnChars: '字数',
    columnExcerpt: '内容开头',
    columnAction: '正文',
    statusPresent: '发送',
    statusEmpty: '没有文字',
    statusUnsupported: '这种格式读不出文字',
    statusFailed: '提取失败',
    statusExcluded: '已关闭',
    excludeHint: '点「发送」可以关掉某个文件的正文：文件名仍会发送，只是不带内容。',
    textTruncated: '（已截断）',
    emptyExcerpt: '（空）',
    grantAction: '确认并授权发送',
    granting: '正在授权…',
    grantedNotice: '已授权本次内容。改动要求或文件后需要重新授权。',
    needGrant: '云端模式需要先确认要发送的内容。请点「确认并授权发送」。',
    startAction: '开始分析',
    starting: '正在请求模型…',
    running: '模型正在处理…',
    cancelHint: '分析进行中，可以稍候。',
    localNotice: '本机模型：内容不会离开这台电脑，所以不需要额外授权。',
    cloudNotice: '云端模型：内容会发送到提供商，需要你先确认。',
    notPreviewed: '还没有准备待发送内容。',
    done: '分析完成。',
    buildFromAnalysis: '按分析结果生成计划',
    building: '正在生成计划…',
  },
  preview: {
    search: '搜索文件或目标路径',
    filterLabel: '显示范围',
    deselectFiltered: '取消筛选项的选择',
    filterEmpty: '没有匹配的文件。试试其他关键词或显示范围。',
    filterCount: (shown: number, total: number) => `显示 ${shown} / ${total} 项`,
    filterHint: '筛选只改变显示范围，隐藏项的选择会保留。批量取消后需应用修改。',
    reasonHeading: '整理理由',
    heading: '整理预览',
    description:
      '下面是计划中的所有变更。逐条确认后再执行；执行前还会重新核对每个文件，发现变化会立即停止。',
    notReady: '还没有可预览的计划。请先扫描一个文件夹。',

    // 概览
    summarySelected: '已选中',
    summaryExecutable: '可执行',
    summaryBlocked: '有阻断问题',
    summaryUnit: '项',

    // 表格
    columnSelect: '选择',
    columnSource: '当前位置',
    columnTarget: '整理后',
    columnAction: '动作',
    columnOrigin: '来源',
    actionMove: '移动',
    actionNoop: '保持原样',
    originRule: '规则',
    originAi: '模型建议',
    originUser: '手动',

    // 编辑与筛选
    editTarget: '编辑目标位置',
    editHint: '只能修改目录与文件名，不能改变扩展名。',
    applyEdit: '应用',
    cancelEdit: '取消',
    discardEdits: '放弃全部修改',
    hasPendingEdits: '有未保存的修改',
    filterAll: '全部',
    filterConflicts: '仅看有问题的',
    filterSelected: '仅看已选中的',
    selectAll: '全选',
    selectNone: '全不选',

    // 问题
    issuesHeading: '需要处理的问题',
    noIssues: '没有发现问题。',
    globalIssue: '全局',

    // 确认
    confirmHeading: '确认执行',
    confirmSummaryPrefix: '即将整理',
    confirmSummaryUnit: '个文件，根目录为',
    confirmIrreversible: '执行会真实移动文件。请确认逐条内容无误。',
    confirmButton: '确认并执行',
    // 对话框里的按钮与页面上的主按钮**必须不同名**：
    // 同名会让「点主按钮打开对话框、再点对话框按钮」这条路径无法用角色查询定位。
    confirmInDialog: '确认执行',
    blockedCannotConfirm: '存在阻断项，无法执行。请先解决上面的问题。',
    nothingSelected: '没有选中任何项。',
    staleAfterEdit: '计划已被修改，请重新预览后再确认。',
    needValidate: '请先点「校验并获取确认」，再执行确认。',
    saveBeforeValidate: '有尚未保存的修改。请先应用或放弃修改，再校验并确认。',
    tokenExpired: '确认已超过 5 分钟有效期，请重新预览并确认。',
    tokenUsed: '该确认已被使用过，不能重复执行。',
    clearConfirmation: '清除确认',

    // 本阶段不接通执行
    validateButton: '校验并获取确认',
    validating: '正在校验…',

    // 执行
    executeButton: '执行整理',
    executing: '正在整理…',
    executeNotAvailable:
      '执行会真实移动文件。请确认列表内容无误后再执行；执行过程中每一项都会重新核对，发现变化立即停止。',
    runHeading: '执行结果',
    runCompleted: '全部完成',
    runPartial: '部分完成',
    runFailed: '未能执行',
    runRecoveryRequired: '需要人工核对',
    runApplied: '已移动',
    runFailedCount: '失败',
    runSkipped: '未执行',
    runPending: '未完成',
    runDeduplicated: '这次请求之前已经执行过，以下是那次的结果。',
    executeSucceeded: '整理已完成。',
    cancelExecution: '停止整理',
    cancelling: '正在停止…',
    cancelRequested:
      '已发出停止请求。正在处理的那一项会先完成（半途而废会留下无法判定的状态），之后的项不会再动。',
    progressLabel: '已整理',
    // 进度区域的**可访问名**。
    //
    // 页面上有三处 `role="status"`：进度面板、禁用理由列表、停止请求提示。
    // 读屏用户听到的是三段无名公告，分不清哪句在说「正在做什么」。
    // 给进度面板一个可访问名之后，它成为一个可被点名定位的区域——
    // 「整理进度」这个区域在说话，而不是又冒出一句无主的话。
    progressRegionLabel: '整理进度',
  },

  recovery: {
    heading: '恢复核对',
    description:
      '上次整理没有正常结束。下面每一项的判定都来自**当前磁盘上的实际状态**，应用不会替你猜测。核对清楚后，如果决定保留现状，请写明理由并确认。',
    // 入口
    noRun: '没有需要核对的执行记录。',
    noRunHint: '从「历史」里选一条标记为「需要人工核对」的记录进入这里。',
    // 概况
    blocking: '还有未核对完的项。在全部核对或明确接受之前，应用不接受新的整理任务。',
    clear: '所有未决项都已核对完毕，可以开始新的整理了。',
    rootUnauthorized:
      '这个文件夹的授权已经失效，暂时无法核对磁盘。请重新在首页选择同一个文件夹，然后再回来核对。',
    recheck: '重新核对',
    // 未决项
    pendingHeading: '待核对',
    pendingEmpty: '没有待核对的项。',
    sourceLabel: '原位置',
    targetLabel: '整理后',
    // 确认
    acknowledgeHeading: '保留现状并确认知晓',
    acknowledgeHint:
      '确认不会删除、移动或覆盖任何文件——两边的文件都原样留着。这一步只是记下「你已经看过，并接受这个结果」。',
    reasonLabel: '理由（必填）',
    reasonPlaceholder: '例如：原位置那份还要留着，目标位置那份请忽略',
    reasonRequired: '请先写明理由，便于日后追溯。',
    acknowledgeButton: '确认保留现状',
    acknowledging: '正在记录…',
    acknowledgeDisabled: '文件夹授权失效时无法核对，因此也不能确认。请先重新选择文件夹。',
    stale: '磁盘状态在核对之后又变了，已重新读取，请再看一遍再确认。',
    acknowledged: '已记录。所有未决项都处理完了，现在可以开始新的整理。',
    acknowledgedStillBlocked: '已记录这一项。还有别的项没核对完，请继续。',
    // 已确认
    settledHeading: '已确认保留现状',
    settledNote: '这一项你已经确认过。磁盘上的文件没有被改动，两处内容仍在原处。',
  },

  undo: {
    heading: '撤销整理',
    description:
      '撤销会把文件**搬回整理之前的位置**。它和整理一样要先看清清单再确认，不是「出错了就点一下」的按钮。',
    // 入口
    noRun: '没有可以撤销的整理记录。',
    noRunHint: '从「历史」里选一条已完成的整理记录，点它的「撤销」入口。',
    openFromHistory: '撤销',
    undoRecordLabel: '撤销记录',
    applyRecordLabel: '整理记录',
    // 概况
    summary: (ready: number, conflict: number, undone: number) =>
      `可以撤销 ${ready} 项，有冲突 ${conflict} 项，已经撤销过 ${undone} 项。`,
    allClear: '这一批文件已经全部回到原位置了，没有需要再做的。',
    nothingReady: '没有可以直接撤销的项，下面说明了每一项的原因。',
    recheck: '重新预览',
    // 分项
    readyHeading: '可以撤销',
    conflictHeading: '有冲突（默认不选中）',
    undoneHeading: '已经撤销过',
    sourceLabel: '现在在这里',
    targetLabel: '将搬回',
    conflictNote:
      '有冲突的项**默认不选中**。应用不会覆盖后来出现的文件，也不会搬动整理之后被修改过的文件——那些都要你自己决定怎么处理。',
    undoneNote: '这些项此前已经撤销过，文件已经在原位置，再次撤销不会重复搬动。',
    // 确认
    confirmHeading: '确认撤销',
    confirmHint:
      '只会搬动勾选的那些项。没勾选的、有冲突的，都会原样留在现在的位置。',
    tokenExpiry: '这份预览 5 分钟内有效，过期后请重新预览。',
    executeButton: '确认撤销',
    executing: '正在撤销…',
    cancelExecution: '停止撤销',
    cancelling: '正在停止…',
    cancelRequested: '已发出停止请求。正在处理的文件会先安全完成，后续项将保持原位。',
    progress: (processed: number, total: number) => `撤销进度：${processed} / ${total}`,
    // 结果
    resultHeading: '撤销结果',
    resultReverted: '已撤销',
    resultConflicted: '有冲突',
    resultUntouched: '未处理',
    resultAlreadyUndone: '此前已撤销',
    partialWarning:
      '这次**没有**把所有项都撤回去。上面分项列出了每一项的实际结果，留在原位的文件没有被动过。',
    completed: '全部选中项都已搬回原位置。',
    recoveryRequired:
      '撤销过程中出现了无法立即确定的状态。请先到历史记录中打开恢复核对，不要重复撤销。',
    backToHistory: '返回历史记录',
    warningsHeading: '附带提示',
    warningsNote:
      '这些提示不影响已经搬回的文件——文件本身是安全的，只是有些空目录没能清理。',
    stale: '文件状态在预览之后又变了，已重新读取，请再看一遍再确认。',
    busy: '已经有一个任务在跑，请等它结束再撤销。',
  },

  errors: {
    IPC_UNAVAILABLE:
      '当前不在桌面应用环境中运行，无法调用本地核心。请在 FilePilot 桌面应用内使用。',
    IPC_TRANSPORT_FAILED: '与本地核心通信失败。',
    scanFailed: '扫描任务失败。',
    scanIdMissing: '扫描任务完成但没有返回 scanId。',
    scanTaskMissing: '扫描任务状态已丢失。',
    scanResultMissing: '扫描结果已丢失。',
    unknownCode: '未知错误码',
  },

  a11y: {
    skipToContent: '跳到主要内容',
  },
} as const

export type Messages = typeof zhCN

/** 当前的文案表。多语言在 v0.2 路线里，因此 P0 不做语言切换。 */
export const t: Messages = zhCN
