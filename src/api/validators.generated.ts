// @ts-nocheck
/* eslint-disable */
//
// 本文件由 scripts/generate-validators.mjs 生成，**不要手改**。
//
// 为什么是预编译的：Ajv 运行时编译会 `new Function()`，而生产 CSP 不含
// 'unsafe-eval'（保留严格 CSP 是有意的），安装版会在启动时抛 EvalError 白屏。
// 预编译把「编译」从运行期挪到构建期，运行期只剩静态函数。
//
// 重新生成：pnpm contracts:generate

"use strict";
export const AnalysisStart = validate11;
const schema12 = {"additionalProperties":false,"description":"[`start_analysis`] 的返回。","properties":{"analysisId":{"description":"这次分析的 id。`create_plan` 用它取回建议。","type":"string"},"taskId":{"description":"后台任务的 id（与扫描共用同一套任务通道）。","type":"string"}},"required":["taskId","analysisId"],"type":"object"};

function validate11(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.taskId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "taskId"},message:"must have required property '"+"taskId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.analysisId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "analysisId"},message:"must have required property '"+"analysisId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
for(const key0 in data){
if(!((key0 === "analysisId") || (key0 === "taskId"))){
const err2 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
if(data.analysisId !== undefined){
if(typeof data.analysisId !== "string"){
const err3 = {instancePath:instancePath+"/analysisId",schemaPath:"#/properties/analysisId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.taskId !== undefined){
if(typeof data.taskId !== "string"){
const err4 = {instancePath:instancePath+"/taskId",schemaPath:"#/properties/taskId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
validate11.errors = vErrors;
return errors === 0;
}

export const AppError = validate12;
const schema13 = {"additionalProperties":false,"description":"返回给前端的脱敏错误。\n\n`details` **不得**包含文件正文、绝对路径或密钥（规格 3.3、5.1）。\n\n`deny_unknown_fields` 同时做两件事：让 Rust 反序列化拒绝多余字段，\n以及让 schemars 生成 `additionalProperties: false`（规格 6.4 的硬要求）。","properties":{"code":{"type":"string"},"details":{"additionalProperties":{"type":"string"},"type":"object"},"message":{"type":"string"},"retryable":{"description":"指明「重试一次可能成功」。为 `true` 时前端才显示重试入口。","type":"boolean"}},"required":["code","message","retryable","details"],"type":"object"};

function validate12(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.code === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "code"},message:"must have required property '"+"code"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.message === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.retryable === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "retryable"},message:"must have required property '"+"retryable"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.details === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "details"},message:"must have required property '"+"details"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
for(const key0 in data){
if(!((((key0 === "code") || (key0 === "details")) || (key0 === "message")) || (key0 === "retryable"))){
const err4 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
if(data.code !== undefined){
if(typeof data.code !== "string"){
const err5 = {instancePath:instancePath+"/code",schemaPath:"#/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
if(data.details !== undefined){
let data1 = data.details;
if(data1 && typeof data1 == "object" && !Array.isArray(data1)){
for(const key1 in data1){
if(typeof data1[key1] !== "string"){
const err6 = {instancePath:instancePath+"/details/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"),schemaPath:"#/properties/details/additionalProperties/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
}
else {
const err7 = {instancePath:instancePath+"/details",schemaPath:"#/properties/details/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.message !== undefined){
if(typeof data.message !== "string"){
const err8 = {instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.retryable !== undefined){
if(typeof data.retryable !== "boolean"){
const err9 = {instancePath:instancePath+"/retryable",schemaPath:"#/properties/retryable/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
}
else {
const err10 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
validate12.errors = vErrors;
return errors === 0;
}

export const AppSettings = validate13;
const schema14 = {"additionalProperties":false,"description":"非敏感应用设置。\n\n规格 3.3 与 8.1：**禁止**在这里放密钥。API Key 只进 Windows 凭据存储，\n设置中只保存 `selected_provider_id` 这类引用。","properties":{"mode":{"description":"整理模式。","oneOf":[{"const":"rules","description":"纯规则模式：不联网、不做内容提取，断网与无模型时仍然完整可用。","type":"string"},{"const":"aiLocal","description":"本地模型模式：只允许 loopback 地址。","type":"string"},{"const":"aiCloud","description":"云端模型模式：必须先展示并授权真实待发送载荷。","type":"string"}]},"scanMaxDepth":{"description":"规格 6.1 默认值：递归深度 20。","format":"uint32","minimum":0,"type":"integer"},"scanMaxFiles":{"description":"规格 6.1 默认值：最多 10,000 个普通文件。","format":"uint32","minimum":0,"type":"integer"},"selectedProviderId":{"type":["string","null"]}},"required":["mode","scanMaxFiles","scanMaxDepth","selectedProviderId"],"type":"object"};

function validate13(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.mode === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "mode"},message:"must have required property '"+"mode"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.scanMaxFiles === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "scanMaxFiles"},message:"must have required property '"+"scanMaxFiles"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.scanMaxDepth === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "scanMaxDepth"},message:"must have required property '"+"scanMaxDepth"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.selectedProviderId === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "selectedProviderId"},message:"must have required property '"+"selectedProviderId"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
for(const key0 in data){
if(!((((key0 === "mode") || (key0 === "scanMaxDepth")) || (key0 === "scanMaxFiles")) || (key0 === "selectedProviderId"))){
const err4 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
if(data.mode !== undefined){
let data0 = data.mode;
const _errs3 = errors;
let valid1 = false;
let passing0 = null;
const _errs4 = errors;
if(typeof data0 !== "string"){
const err5 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if("rules" !== data0){
const err6 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/0/const",keyword:"const",params:{allowedValue: "rules"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
var _valid0 = _errs4 === errors;
if(_valid0){
valid1 = true;
passing0 = 0;
}
const _errs6 = errors;
if(typeof data0 !== "string"){
const err7 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if("aiLocal" !== data0){
const err8 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/1/const",keyword:"const",params:{allowedValue: "aiLocal"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
var _valid0 = _errs6 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid1 = true;
passing0 = 1;
}
const _errs8 = errors;
if(typeof data0 !== "string"){
const err9 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
if("aiCloud" !== data0){
const err10 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/2/const",keyword:"const",params:{allowedValue: "aiCloud"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
var _valid0 = _errs8 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid1 = true;
passing0 = 2;
}
}
}
if(!valid1){
const err11 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
else {
errors = _errs3;
if(vErrors !== null){
if(_errs3){
vErrors.length = _errs3;
}
else {
vErrors = null;
}
}
}
}
if(data.scanMaxDepth !== undefined){
let data1 = data.scanMaxDepth;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err12 = {instancePath:instancePath+"/scanMaxDepth",schemaPath:"#/properties/scanMaxDepth/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err13 = {instancePath:instancePath+"/scanMaxDepth",schemaPath:"#/properties/scanMaxDepth/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
}
if(data.scanMaxFiles !== undefined){
let data2 = data.scanMaxFiles;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
const err14 = {instancePath:instancePath+"/scanMaxFiles",schemaPath:"#/properties/scanMaxFiles/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
const err15 = {instancePath:instancePath+"/scanMaxFiles",schemaPath:"#/properties/scanMaxFiles/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
}
}
if(data.selectedProviderId !== undefined){
let data3 = data.selectedProviderId;
if((typeof data3 !== "string") && (data3 !== null)){
const err16 = {instancePath:instancePath+"/selectedProviderId",schemaPath:"#/properties/selectedProviderId/type",keyword:"type",params:{type: schema14.properties.selectedProviderId.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
}
else {
const err17 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
validate13.errors = vErrors;
return errors === 0;
}

export const DisclosureGrant = validate14;
const schema15 = {"additionalProperties":false,"description":"`grant_disclosure` 的返回。","properties":{"consentId":{"type":"string"},"payloadDigest":{"type":"string"}},"required":["consentId","payloadDigest"],"type":"object"};

function validate14(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.consentId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "consentId"},message:"must have required property '"+"consentId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.payloadDigest === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "payloadDigest"},message:"must have required property '"+"payloadDigest"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
for(const key0 in data){
if(!((key0 === "consentId") || (key0 === "payloadDigest"))){
const err2 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
if(data.consentId !== undefined){
if(typeof data.consentId !== "string"){
const err3 = {instancePath:instancePath+"/consentId",schemaPath:"#/properties/consentId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.payloadDigest !== undefined){
if(typeof data.payloadDigest !== "string"){
const err4 = {instancePath:instancePath+"/payloadDigest",schemaPath:"#/properties/payloadDigest/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
validate14.errors = vErrors;
return errors === 0;
}

export const DisclosureItemPreview = validate15;
const schema16 = {"additionalProperties":false,"description":"预览里逐文件的一项。","properties":{"characterCount":{"format":"uint32","minimum":0,"type":"integer"},"excerpt":{"description":"实际文本的开头。","type":"string"},"fileId":{"type":"string"},"fileName":{"type":"string"},"textStatus":{"description":"这一项**有没有可发送的正文**，以及为什么没有。\n\n少了它，界面只能显示「0 个字符」，而用户从那个数字里读不出\n该做什么：\n\n| 情况 | 用户该做的事 |\n|---|---|\n| 文件本来就没有文字 | 换一个文件 |\n| 格式读不出文字（扫描型 PDF） | 找它的文字版 |\n| **提取失败** | 重试，或者查这台机器上的解析环境 |\n| 用户自己关掉了正文 | 没别的，就是他选的 |\n\n把它们混成一句「没有正文」，用户会去检查一个完全正常的文件，\n或者白白放弃一个其实能用的文件。","oneOf":[{"const":"present","description":"有正文可以发送。","type":"string"},{"const":"empty","description":"提取成功，但内容里确实没有可读文本。","type":"string"},{"const":"unsupported","description":"格式不受支持（例如扫描型 PDF 没有文本层）。","type":"string"},{"const":"failed","description":"**提取失败**：本该读出来却没读成。这是最需要用户知道的一种。","type":"string"},{"const":"excluded","description":"用户关掉了这个文件的正文（规格：「预览中允许排除某个文件/关闭其正文」）。","type":"string"},{"const":"pending","description":"还没提取过。正常情况下不该出现在预览里。","type":"string"}]},"truncated":{"description":"这段文本被裁剪过（原文更长）。\n\n规格原话：「**文本裁剪不是隐私脱敏保证**」——它只是控制载荷大小。\n界面上要如实说明「被截过」，免得用户以为剩下的内容不会发出去。","type":"boolean"}},"required":["fileId","fileName","characterCount","truncated","excerpt","textStatus"],"type":"object"};

function validate15(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.fileId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "fileId"},message:"must have required property '"+"fileId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.fileName === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "fileName"},message:"must have required property '"+"fileName"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.characterCount === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "characterCount"},message:"must have required property '"+"characterCount"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.truncated === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "truncated"},message:"must have required property '"+"truncated"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.excerpt === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "excerpt"},message:"must have required property '"+"excerpt"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.textStatus === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "textStatus"},message:"must have required property '"+"textStatus"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
for(const key0 in data){
if(!((((((key0 === "characterCount") || (key0 === "excerpt")) || (key0 === "fileId")) || (key0 === "fileName")) || (key0 === "textStatus")) || (key0 === "truncated"))){
const err6 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
if(data.characterCount !== undefined){
let data0 = data.characterCount;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
const err7 = {instancePath:instancePath+"/characterCount",schemaPath:"#/properties/characterCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
const err8 = {instancePath:instancePath+"/characterCount",schemaPath:"#/properties/characterCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
}
if(data.excerpt !== undefined){
if(typeof data.excerpt !== "string"){
const err9 = {instancePath:instancePath+"/excerpt",schemaPath:"#/properties/excerpt/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.fileId !== undefined){
if(typeof data.fileId !== "string"){
const err10 = {instancePath:instancePath+"/fileId",schemaPath:"#/properties/fileId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
if(data.fileName !== undefined){
if(typeof data.fileName !== "string"){
const err11 = {instancePath:instancePath+"/fileName",schemaPath:"#/properties/fileName/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
if(data.textStatus !== undefined){
let data4 = data.textStatus;
const _errs11 = errors;
let valid1 = false;
let passing0 = null;
const _errs12 = errors;
if(typeof data4 !== "string"){
const err12 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if("present" !== data4){
const err13 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/0/const",keyword:"const",params:{allowedValue: "present"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
var _valid0 = _errs12 === errors;
if(_valid0){
valid1 = true;
passing0 = 0;
}
const _errs14 = errors;
if(typeof data4 !== "string"){
const err14 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if("empty" !== data4){
const err15 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/1/const",keyword:"const",params:{allowedValue: "empty"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
var _valid0 = _errs14 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid1 = true;
passing0 = 1;
}
const _errs16 = errors;
if(typeof data4 !== "string"){
const err16 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if("unsupported" !== data4){
const err17 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/2/const",keyword:"const",params:{allowedValue: "unsupported"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
var _valid0 = _errs16 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid1 = true;
passing0 = 2;
}
const _errs18 = errors;
if(typeof data4 !== "string"){
const err18 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if("failed" !== data4){
const err19 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/3/const",keyword:"const",params:{allowedValue: "failed"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
var _valid0 = _errs18 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid1 = true;
passing0 = 3;
}
const _errs20 = errors;
if(typeof data4 !== "string"){
const err20 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/4/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if("excluded" !== data4){
const err21 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/4/const",keyword:"const",params:{allowedValue: "excluded"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
var _valid0 = _errs20 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid1 = true;
passing0 = 4;
}
const _errs22 = errors;
if(typeof data4 !== "string"){
const err22 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/5/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if("pending" !== data4){
const err23 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf/5/const",keyword:"const",params:{allowedValue: "pending"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
var _valid0 = _errs22 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 5];
}
else {
if(_valid0){
valid1 = true;
passing0 = 5;
}
}
}
}
}
}
if(!valid1){
const err24 = {instancePath:instancePath+"/textStatus",schemaPath:"#/properties/textStatus/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
else {
errors = _errs11;
if(vErrors !== null){
if(_errs11){
vErrors.length = _errs11;
}
else {
vErrors = null;
}
}
}
}
if(data.truncated !== undefined){
if(typeof data.truncated !== "boolean"){
const err25 = {instancePath:instancePath+"/truncated",schemaPath:"#/properties/truncated/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
}
else {
const err26 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
validate15.errors = vErrors;
return errors === 0;
}

export const DisclosurePreview = validate16;
const schema17 = {"additionalProperties":false,"description":"`preview_disclosure` 的返回：**只在本机准备的**一份待发送载荷。","properties":{"characterCount":{"format":"uint64","minimum":0,"type":"integer"},"fileCount":{"format":"uint32","minimum":0,"type":"integer"},"instruction":{"type":"string"},"items":{"items":{"additionalProperties":false,"description":"预览里逐文件的一项。","properties":{"characterCount":{"format":"uint32","minimum":0,"type":"integer"},"excerpt":{"description":"实际文本的开头。","type":"string"},"fileId":{"type":"string"},"fileName":{"type":"string"},"textStatus":{"description":"这一项**有没有可发送的正文**，以及为什么没有。\n\n少了它，界面只能显示「0 个字符」，而用户从那个数字里读不出\n该做什么：\n\n| 情况 | 用户该做的事 |\n|---|---|\n| 文件本来就没有文字 | 换一个文件 |\n| 格式读不出文字（扫描型 PDF） | 找它的文字版 |\n| **提取失败** | 重试，或者查这台机器上的解析环境 |\n| 用户自己关掉了正文 | 没别的，就是他选的 |\n\n把它们混成一句「没有正文」，用户会去检查一个完全正常的文件，\n或者白白放弃一个其实能用的文件。","oneOf":[{"const":"present","description":"有正文可以发送。","type":"string"},{"const":"empty","description":"提取成功，但内容里确实没有可读文本。","type":"string"},{"const":"unsupported","description":"格式不受支持（例如扫描型 PDF 没有文本层）。","type":"string"},{"const":"failed","description":"**提取失败**：本该读出来却没读成。这是最需要用户知道的一种。","type":"string"},{"const":"excluded","description":"用户关掉了这个文件的正文（规格：「预览中允许排除某个文件/关闭其正文」）。","type":"string"},{"const":"pending","description":"还没提取过。正常情况下不该出现在预览里。","type":"string"}]},"truncated":{"description":"这段文本被裁剪过（原文更长）。\n\n规格原话：「**文本裁剪不是隐私脱敏保证**」——它只是控制载荷大小。\n界面上要如实说明「被截过」，免得用户以为剩下的内容不会发出去。","type":"boolean"}},"required":["fileId","fileName","characterCount","truncated","excerpt","textStatus"],"type":"object"},"type":"array"},"model":{"type":"string"},"payloadDigest":{"description":"这份载荷的摘要。`grant_disclosure` 用它换授权。","type":"string"},"providerId":{"type":"string"}},"required":["payloadDigest","providerId","model","fileCount","characterCount","instruction","items"],"type":"object"};

function validate16(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.payloadDigest === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "payloadDigest"},message:"must have required property '"+"payloadDigest"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.providerId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "providerId"},message:"must have required property '"+"providerId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.model === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "model"},message:"must have required property '"+"model"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.fileCount === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "fileCount"},message:"must have required property '"+"fileCount"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.characterCount === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "characterCount"},message:"must have required property '"+"characterCount"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.instruction === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "instruction"},message:"must have required property '"+"instruction"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.items === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "items"},message:"must have required property '"+"items"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key0 in data){
if(!(((((((key0 === "characterCount") || (key0 === "fileCount")) || (key0 === "instruction")) || (key0 === "items")) || (key0 === "model")) || (key0 === "payloadDigest")) || (key0 === "providerId"))){
const err7 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.characterCount !== undefined){
let data0 = data.characterCount;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
const err8 = {instancePath:instancePath+"/characterCount",schemaPath:"#/properties/characterCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
const err9 = {instancePath:instancePath+"/characterCount",schemaPath:"#/properties/characterCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
}
if(data.fileCount !== undefined){
let data1 = data.fileCount;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err10 = {instancePath:instancePath+"/fileCount",schemaPath:"#/properties/fileCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err11 = {instancePath:instancePath+"/fileCount",schemaPath:"#/properties/fileCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
}
if(data.instruction !== undefined){
if(typeof data.instruction !== "string"){
const err12 = {instancePath:instancePath+"/instruction",schemaPath:"#/properties/instruction/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
}
if(data.items !== undefined){
let data3 = data.items;
if(Array.isArray(data3)){
const len0 = data3.length;
for(let i0=0; i0<len0; i0++){
let data4 = data3[i0];
if(data4 && typeof data4 == "object" && !Array.isArray(data4)){
if(data4.fileId === undefined){
const err13 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "fileId"},message:"must have required property '"+"fileId"+"'"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if(data4.fileName === undefined){
const err14 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "fileName"},message:"must have required property '"+"fileName"+"'"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if(data4.characterCount === undefined){
const err15 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "characterCount"},message:"must have required property '"+"characterCount"+"'"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
if(data4.truncated === undefined){
const err16 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "truncated"},message:"must have required property '"+"truncated"+"'"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if(data4.excerpt === undefined){
const err17 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "excerpt"},message:"must have required property '"+"excerpt"+"'"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
if(data4.textStatus === undefined){
const err18 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "textStatus"},message:"must have required property '"+"textStatus"+"'"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
for(const key1 in data4){
if(!((((((key1 === "characterCount") || (key1 === "excerpt")) || (key1 === "fileId")) || (key1 === "fileName")) || (key1 === "textStatus")) || (key1 === "truncated"))){
const err19 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
}
if(data4.characterCount !== undefined){
let data5 = data4.characterCount;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
const err20 = {instancePath:instancePath+"/items/" + i0+"/characterCount",schemaPath:"#/properties/items/items/properties/characterCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
const err21 = {instancePath:instancePath+"/items/" + i0+"/characterCount",schemaPath:"#/properties/items/items/properties/characterCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
}
if(data4.excerpt !== undefined){
if(typeof data4.excerpt !== "string"){
const err22 = {instancePath:instancePath+"/items/" + i0+"/excerpt",schemaPath:"#/properties/items/items/properties/excerpt/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
}
if(data4.fileId !== undefined){
if(typeof data4.fileId !== "string"){
const err23 = {instancePath:instancePath+"/items/" + i0+"/fileId",schemaPath:"#/properties/items/items/properties/fileId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
}
if(data4.fileName !== undefined){
if(typeof data4.fileName !== "string"){
const err24 = {instancePath:instancePath+"/items/" + i0+"/fileName",schemaPath:"#/properties/items/items/properties/fileName/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
}
if(data4.textStatus !== undefined){
let data9 = data4.textStatus;
const _errs22 = errors;
let valid4 = false;
let passing0 = null;
const _errs23 = errors;
if(typeof data9 !== "string"){
const err25 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
if("present" !== data9){
const err26 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/0/const",keyword:"const",params:{allowedValue: "present"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
var _valid0 = _errs23 === errors;
if(_valid0){
valid4 = true;
passing0 = 0;
}
const _errs25 = errors;
if(typeof data9 !== "string"){
const err27 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
if("empty" !== data9){
const err28 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/1/const",keyword:"const",params:{allowedValue: "empty"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
var _valid0 = _errs25 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid4 = true;
passing0 = 1;
}
const _errs27 = errors;
if(typeof data9 !== "string"){
const err29 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
if("unsupported" !== data9){
const err30 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/2/const",keyword:"const",params:{allowedValue: "unsupported"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
var _valid0 = _errs27 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid4 = true;
passing0 = 2;
}
const _errs29 = errors;
if(typeof data9 !== "string"){
const err31 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
if("failed" !== data9){
const err32 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/3/const",keyword:"const",params:{allowedValue: "failed"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
var _valid0 = _errs29 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid4 = true;
passing0 = 3;
}
const _errs31 = errors;
if(typeof data9 !== "string"){
const err33 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/4/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
if("excluded" !== data9){
const err34 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/4/const",keyword:"const",params:{allowedValue: "excluded"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
var _valid0 = _errs31 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid4 = true;
passing0 = 4;
}
const _errs33 = errors;
if(typeof data9 !== "string"){
const err35 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/5/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
if("pending" !== data9){
const err36 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf/5/const",keyword:"const",params:{allowedValue: "pending"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
var _valid0 = _errs33 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 5];
}
else {
if(_valid0){
valid4 = true;
passing0 = 5;
}
}
}
}
}
}
if(!valid4){
const err37 = {instancePath:instancePath+"/items/" + i0+"/textStatus",schemaPath:"#/properties/items/items/properties/textStatus/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
else {
errors = _errs22;
if(vErrors !== null){
if(_errs22){
vErrors.length = _errs22;
}
else {
vErrors = null;
}
}
}
}
if(data4.truncated !== undefined){
if(typeof data4.truncated !== "boolean"){
const err38 = {instancePath:instancePath+"/items/" + i0+"/truncated",schemaPath:"#/properties/items/items/properties/truncated/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
}
}
else {
const err39 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
}
}
else {
const err40 = {instancePath:instancePath+"/items",schemaPath:"#/properties/items/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
}
if(data.model !== undefined){
if(typeof data.model !== "string"){
const err41 = {instancePath:instancePath+"/model",schemaPath:"#/properties/model/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
}
if(data.payloadDigest !== undefined){
if(typeof data.payloadDigest !== "string"){
const err42 = {instancePath:instancePath+"/payloadDigest",schemaPath:"#/properties/payloadDigest/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
}
if(data.providerId !== undefined){
if(typeof data.providerId !== "string"){
const err43 = {instancePath:instancePath+"/providerId",schemaPath:"#/properties/providerId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
}
}
else {
const err44 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
validate16.errors = vErrors;
return errors === 0;
}

export const DisclosureTextStatus = validate17;
const schema18 = {"description":"预览里这一项的正文状态。","oneOf":[{"const":"present","description":"有正文可以发送。","type":"string"},{"const":"empty","description":"提取成功，但内容里确实没有可读文本。","type":"string"},{"const":"unsupported","description":"格式不受支持（例如扫描型 PDF 没有文本层）。","type":"string"},{"const":"failed","description":"**提取失败**：本该读出来却没读成。这是最需要用户知道的一种。","type":"string"},{"const":"excluded","description":"用户关掉了这个文件的正文（规格：「预览中允许排除某个文件/关闭其正文」）。","type":"string"},{"const":"pending","description":"还没提取过。正常情况下不该出现在预览里。","type":"string"}]};

function validate17(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("present" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "present"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("empty" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "empty"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
const _errs5 = errors;
if(typeof data !== "string"){
const err4 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if("unsupported" !== data){
const err5 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "unsupported"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
const _errs7 = errors;
if(typeof data !== "string"){
const err6 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if("failed" !== data){
const err7 = {instancePath,schemaPath:"#/oneOf/3/const",keyword:"const",params:{allowedValue: "failed"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
var _valid0 = _errs7 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
}
const _errs9 = errors;
if(typeof data !== "string"){
const err8 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if("excluded" !== data){
const err9 = {instancePath,schemaPath:"#/oneOf/4/const",keyword:"const",params:{allowedValue: "excluded"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
}
const _errs11 = errors;
if(typeof data !== "string"){
const err10 = {instancePath,schemaPath:"#/oneOf/5/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if("pending" !== data){
const err11 = {instancePath,schemaPath:"#/oneOf/5/const",keyword:"const",params:{allowedValue: "pending"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
var _valid0 = _errs11 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 5];
}
else {
if(_valid0){
valid0 = true;
passing0 = 5;
}
}
}
}
}
}
if(!valid0){
const err12 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate17.errors = vErrors;
return errors === 0;
}

export const FilePage = validate18;
const schema19 = {"additionalProperties":false,"description":"文件列表的一页。","properties":{"items":{"items":{"additionalProperties":false,"description":"扫描到的文件记录。","properties":{"extension":{"type":"string"},"extractionStatus":{"description":"内容提取状态。","oneOf":[{"enum":["pending","ok","failed"],"type":"string"},{"const":"partial","description":"部分提取（例如超出字符上限被截断）。","type":"string"},{"const":"unsupported","description":"格式不受支持（例如扫描型 PDF 无文本层）。","type":"string"}]},"fingerprint":{"additionalProperties":false,"description":"文件指纹。\n\n字段用途不同，不能互相替代：`file_id` 是 Windows 卷内身份（能检出「同名另一个文件」），\n`sha256` 是内容身份（能检出「size 与 mtime 恰好相同的篡改」）。","properties":{"fileId":{"description":"Windows 文件身份（卷内唯一）。不等同于路径。","type":"string"},"modifiedNs":{"description":"UTC 纳秒时间戳，十进制字符串。","type":"string"},"sha256":{"description":"生成**可执行计划**时必须为 `Some`。仅用于展示的初扫结果可以为 `None`。","type":["string","null"]},"size":{"description":"十进制字符串，避免 JavaScript 数值精度损失。","type":"string"},"volumeId":{"type":"string"}},"required":["volumeId","fileId","size","modifiedNs","sha256"],"type":"object"},"id":{"type":"string"},"relativePath":{"items":{"type":"string"},"type":"array"},"rootId":{"type":"string"},"scanId":{"type":"string"},"skipCode":{"description":"跳过原因代码；`None` 表示未被跳过。","type":["string","null"]}},"required":["id","scanId","rootId","relativePath","extension","fingerprint","extractionStatus","skipCode"],"type":"object"},"type":"array"},"nextCursor":{"description":"下一页游标；为 `None` 表示已经是最后一页。\n\n游标是**上一页最后一条记录的 id**，不是偏移量：\n偏移量在底层列表变化时会漏项或重复，id 不会。","type":["string","null"]},"total":{"description":"该 scan 下的条目总数，便于界面显示「x / total」。","format":"uint32","minimum":0,"type":"integer"}},"required":["items","nextCursor","total"],"type":"object"};

function validate18(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.items === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "items"},message:"must have required property '"+"items"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.nextCursor === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "nextCursor"},message:"must have required property '"+"nextCursor"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.total === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "total"},message:"must have required property '"+"total"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
for(const key0 in data){
if(!(((key0 === "items") || (key0 === "nextCursor")) || (key0 === "total"))){
const err3 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.items !== undefined){
let data0 = data.items;
if(Array.isArray(data0)){
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
let data1 = data0[i0];
if(data1 && typeof data1 == "object" && !Array.isArray(data1)){
if(data1.id === undefined){
const err4 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "id"},message:"must have required property '"+"id"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data1.scanId === undefined){
const err5 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "scanId"},message:"must have required property '"+"scanId"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data1.rootId === undefined){
const err6 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "rootId"},message:"must have required property '"+"rootId"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if(data1.relativePath === undefined){
const err7 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "relativePath"},message:"must have required property '"+"relativePath"+"'"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(data1.extension === undefined){
const err8 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "extension"},message:"must have required property '"+"extension"+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if(data1.fingerprint === undefined){
const err9 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "fingerprint"},message:"must have required property '"+"fingerprint"+"'"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
if(data1.extractionStatus === undefined){
const err10 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "extractionStatus"},message:"must have required property '"+"extractionStatus"+"'"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if(data1.skipCode === undefined){
const err11 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "skipCode"},message:"must have required property '"+"skipCode"+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
for(const key1 in data1){
if(!((((((((key1 === "extension") || (key1 === "extractionStatus")) || (key1 === "fingerprint")) || (key1 === "id")) || (key1 === "relativePath")) || (key1 === "rootId")) || (key1 === "scanId")) || (key1 === "skipCode"))){
const err12 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
}
if(data1.extension !== undefined){
if(typeof data1.extension !== "string"){
const err13 = {instancePath:instancePath+"/items/" + i0+"/extension",schemaPath:"#/properties/items/items/properties/extension/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
if(data1.extractionStatus !== undefined){
let data3 = data1.extractionStatus;
const _errs10 = errors;
let valid4 = false;
let passing0 = null;
const _errs11 = errors;
if(typeof data3 !== "string"){
const err14 = {instancePath:instancePath+"/items/" + i0+"/extractionStatus",schemaPath:"#/properties/items/items/properties/extractionStatus/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if(!(((data3 === "pending") || (data3 === "ok")) || (data3 === "failed"))){
const err15 = {instancePath:instancePath+"/items/" + i0+"/extractionStatus",schemaPath:"#/properties/items/items/properties/extractionStatus/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema19.properties.items.items.properties.extractionStatus.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
var _valid0 = _errs11 === errors;
if(_valid0){
valid4 = true;
passing0 = 0;
}
const _errs13 = errors;
if(typeof data3 !== "string"){
const err16 = {instancePath:instancePath+"/items/" + i0+"/extractionStatus",schemaPath:"#/properties/items/items/properties/extractionStatus/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if("partial" !== data3){
const err17 = {instancePath:instancePath+"/items/" + i0+"/extractionStatus",schemaPath:"#/properties/items/items/properties/extractionStatus/oneOf/1/const",keyword:"const",params:{allowedValue: "partial"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
var _valid0 = _errs13 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid4 = true;
passing0 = 1;
}
const _errs15 = errors;
if(typeof data3 !== "string"){
const err18 = {instancePath:instancePath+"/items/" + i0+"/extractionStatus",schemaPath:"#/properties/items/items/properties/extractionStatus/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if("unsupported" !== data3){
const err19 = {instancePath:instancePath+"/items/" + i0+"/extractionStatus",schemaPath:"#/properties/items/items/properties/extractionStatus/oneOf/2/const",keyword:"const",params:{allowedValue: "unsupported"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
var _valid0 = _errs15 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid4 = true;
passing0 = 2;
}
}
}
if(!valid4){
const err20 = {instancePath:instancePath+"/items/" + i0+"/extractionStatus",schemaPath:"#/properties/items/items/properties/extractionStatus/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
else {
errors = _errs10;
if(vErrors !== null){
if(_errs10){
vErrors.length = _errs10;
}
else {
vErrors = null;
}
}
}
}
if(data1.fingerprint !== undefined){
let data4 = data1.fingerprint;
if(data4 && typeof data4 == "object" && !Array.isArray(data4)){
if(data4.volumeId === undefined){
const err21 = {instancePath:instancePath+"/items/" + i0+"/fingerprint",schemaPath:"#/properties/items/items/properties/fingerprint/required",keyword:"required",params:{missingProperty: "volumeId"},message:"must have required property '"+"volumeId"+"'"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
if(data4.fileId === undefined){
const err22 = {instancePath:instancePath+"/items/" + i0+"/fingerprint",schemaPath:"#/properties/items/items/properties/fingerprint/required",keyword:"required",params:{missingProperty: "fileId"},message:"must have required property '"+"fileId"+"'"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if(data4.size === undefined){
const err23 = {instancePath:instancePath+"/items/" + i0+"/fingerprint",schemaPath:"#/properties/items/items/properties/fingerprint/required",keyword:"required",params:{missingProperty: "size"},message:"must have required property '"+"size"+"'"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
if(data4.modifiedNs === undefined){
const err24 = {instancePath:instancePath+"/items/" + i0+"/fingerprint",schemaPath:"#/properties/items/items/properties/fingerprint/required",keyword:"required",params:{missingProperty: "modifiedNs"},message:"must have required property '"+"modifiedNs"+"'"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if(data4.sha256 === undefined){
const err25 = {instancePath:instancePath+"/items/" + i0+"/fingerprint",schemaPath:"#/properties/items/items/properties/fingerprint/required",keyword:"required",params:{missingProperty: "sha256"},message:"must have required property '"+"sha256"+"'"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
for(const key2 in data4){
if(!(((((key2 === "fileId") || (key2 === "modifiedNs")) || (key2 === "sha256")) || (key2 === "size")) || (key2 === "volumeId"))){
const err26 = {instancePath:instancePath+"/items/" + i0+"/fingerprint",schemaPath:"#/properties/items/items/properties/fingerprint/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
if(data4.fileId !== undefined){
if(typeof data4.fileId !== "string"){
const err27 = {instancePath:instancePath+"/items/" + i0+"/fingerprint/fileId",schemaPath:"#/properties/items/items/properties/fingerprint/properties/fileId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
}
if(data4.modifiedNs !== undefined){
if(typeof data4.modifiedNs !== "string"){
const err28 = {instancePath:instancePath+"/items/" + i0+"/fingerprint/modifiedNs",schemaPath:"#/properties/items/items/properties/fingerprint/properties/modifiedNs/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
}
if(data4.sha256 !== undefined){
let data7 = data4.sha256;
if((typeof data7 !== "string") && (data7 !== null)){
const err29 = {instancePath:instancePath+"/items/" + i0+"/fingerprint/sha256",schemaPath:"#/properties/items/items/properties/fingerprint/properties/sha256/type",keyword:"type",params:{type: schema19.properties.items.items.properties.fingerprint.properties.sha256.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
}
if(data4.size !== undefined){
if(typeof data4.size !== "string"){
const err30 = {instancePath:instancePath+"/items/" + i0+"/fingerprint/size",schemaPath:"#/properties/items/items/properties/fingerprint/properties/size/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
}
if(data4.volumeId !== undefined){
if(typeof data4.volumeId !== "string"){
const err31 = {instancePath:instancePath+"/items/" + i0+"/fingerprint/volumeId",schemaPath:"#/properties/items/items/properties/fingerprint/properties/volumeId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
}
}
else {
const err32 = {instancePath:instancePath+"/items/" + i0+"/fingerprint",schemaPath:"#/properties/items/items/properties/fingerprint/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
}
if(data1.id !== undefined){
if(typeof data1.id !== "string"){
const err33 = {instancePath:instancePath+"/items/" + i0+"/id",schemaPath:"#/properties/items/items/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
}
if(data1.relativePath !== undefined){
let data11 = data1.relativePath;
if(Array.isArray(data11)){
const len1 = data11.length;
for(let i1=0; i1<len1; i1++){
if(typeof data11[i1] !== "string"){
const err34 = {instancePath:instancePath+"/items/" + i0+"/relativePath/" + i1,schemaPath:"#/properties/items/items/properties/relativePath/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
}
}
else {
const err35 = {instancePath:instancePath+"/items/" + i0+"/relativePath",schemaPath:"#/properties/items/items/properties/relativePath/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
}
if(data1.rootId !== undefined){
if(typeof data1.rootId !== "string"){
const err36 = {instancePath:instancePath+"/items/" + i0+"/rootId",schemaPath:"#/properties/items/items/properties/rootId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
}
if(data1.scanId !== undefined){
if(typeof data1.scanId !== "string"){
const err37 = {instancePath:instancePath+"/items/" + i0+"/scanId",schemaPath:"#/properties/items/items/properties/scanId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
}
if(data1.skipCode !== undefined){
let data15 = data1.skipCode;
if((typeof data15 !== "string") && (data15 !== null)){
const err38 = {instancePath:instancePath+"/items/" + i0+"/skipCode",schemaPath:"#/properties/items/items/properties/skipCode/type",keyword:"type",params:{type: schema19.properties.items.items.properties.skipCode.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
}
}
else {
const err39 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
}
}
else {
const err40 = {instancePath:instancePath+"/items",schemaPath:"#/properties/items/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
}
if(data.nextCursor !== undefined){
let data16 = data.nextCursor;
if((typeof data16 !== "string") && (data16 !== null)){
const err41 = {instancePath:instancePath+"/nextCursor",schemaPath:"#/properties/nextCursor/type",keyword:"type",params:{type: schema19.properties.nextCursor.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
}
if(data.total !== undefined){
let data17 = data.total;
if(!(((typeof data17 == "number") && (!(data17 % 1) && !isNaN(data17))) && (isFinite(data17)))){
const err42 = {instancePath:instancePath+"/total",schemaPath:"#/properties/total/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
if((typeof data17 == "number") && (isFinite(data17))){
if(data17 < 0 || isNaN(data17)){
const err43 = {instancePath:instancePath+"/total",schemaPath:"#/properties/total/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
}
}
}
else {
const err44 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
validate18.errors = vErrors;
return errors === 0;
}

export const OcrAvailabilityReport = validate19;
const schema20 = {"description":"工作进程报回来的 OCR 可用状态。\n\n与 `platform::ocr::OcrAvailability` 一一对应，但**独立成一个可序列化的类型**：\n后者带着 WinRT 那侧的语义，不该让 IPC 契约依赖它；而且工作进程与界面进程\n之间走的本来就只有纯数据。\n\n它同时是**设置页要显示的东西**（T11：设置页显示 OCR 可用状态和原因），\n所以进了 IPC 契约，需要和别的契约类型一样可生成 JSON Schema 与 TS 类型。","properties":{"languages":{"description":"可识别的语言标签。不可用时为空或列出已装但不含中文的语言。","items":{"type":"string"},"type":"array"},"message":{"description":"给用户看的一句话，**必须包含下一步动作**。","type":"string"},"status":{"description":"用哪个变体：`available` / `noRecognizerLanguage` / `noChineseLanguage` / `unsupported`。","type":"string"}},"required":["status","languages","message"],"type":"object"};

function validate19(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.status === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.languages === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "languages"},message:"must have required property '"+"languages"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.message === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.languages !== undefined){
let data0 = data.languages;
if(Array.isArray(data0)){
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
if(typeof data0[i0] !== "string"){
const err3 = {instancePath:instancePath+"/languages/" + i0,schemaPath:"#/properties/languages/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
}
else {
const err4 = {instancePath:instancePath+"/languages",schemaPath:"#/properties/languages/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
if(data.message !== undefined){
if(typeof data.message !== "string"){
const err5 = {instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
if(data.status !== undefined){
if(typeof data.status !== "string"){
const err6 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
}
else {
const err7 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
validate19.errors = vErrors;
return errors === 0;
}

export const Plan = validate20;
const schema21 = {"additionalProperties":false,"description":"整理计划。","properties":{"createdAt":{"description":"UTC RFC3339 字符串。","type":"string"},"id":{"type":"string"},"items":{"items":{"additionalProperties":false,"description":"计划中的单项。它已经过确定性规划器处理，带完整源指纹。","properties":{"action":{"description":"单个计划项的动作。v0.1 只有这两种 —— 没有删除、没有覆盖。","enum":["move","noop"],"type":"string"},"expected":{"additionalProperties":false,"description":"生成计划时绑定的源文件指纹。执行前必须重新核对，不一致即停止。","properties":{"fileId":{"description":"Windows 文件身份（卷内唯一）。不等同于路径。","type":"string"},"modifiedNs":{"description":"UTC 纳秒时间戳，十进制字符串。","type":"string"},"sha256":{"description":"生成**可执行计划**时必须为 `Some`。仅用于展示的初扫结果可以为 `None`。","type":["string","null"]},"size":{"description":"十进制字符串，避免 JavaScript 数值精度损失。","type":"string"},"volumeId":{"type":"string"}},"required":["volumeId","fileId","size","modifiedNs","sha256"],"type":"object"},"fileId":{"type":"string"},"id":{"type":"string"},"origin":{"description":"计划项的来源。","enum":["rule","ai","user"],"type":"string"},"reason":{"type":"string"},"selected":{"type":"boolean"},"source":{"items":{"type":"string"},"type":"array"},"target":{"items":{"type":"string"},"type":"array"}},"required":["id","fileId","source","target","action","selected","origin","reason","expected"],"type":"object"},"type":"array"},"mode":{"description":"整理模式。","oneOf":[{"const":"rules","description":"纯规则模式：不联网、不做内容提取，断网与无模型时仍然完整可用。","type":"string"},{"const":"aiLocal","description":"本地模型模式：只允许 loopback 地址。","type":"string"},{"const":"aiCloud","description":"云端模型模式：必须先展示并授权真实待发送载荷。","type":"string"}]},"revision":{"description":"乐观锁版本。任何编辑都会 +1，并使旧校验与旧确认立即失效。","format":"uint32","minimum":0,"type":"integer"},"rootId":{"type":"string"},"scanId":{"type":"string"},"status":{"description":"计划状态。","oneOf":[{"enum":["draft","archived"],"type":"string"},{"const":"validated","description":"已通过校验并取得一次性 validationToken。","type":"string"},{"const":"sealed","description":"已随执行被密封，成为执行输入的唯一来源。","type":"string"}]}},"required":["id","rootId","scanId","revision","mode","status","items","createdAt"],"type":"object"};
const func2 = Object.prototype.hasOwnProperty;

function validate20(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.id === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "id"},message:"must have required property '"+"id"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.rootId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "rootId"},message:"must have required property '"+"rootId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.scanId === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "scanId"},message:"must have required property '"+"scanId"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.revision === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "revision"},message:"must have required property '"+"revision"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.mode === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "mode"},message:"must have required property '"+"mode"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.status === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.items === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "items"},message:"must have required property '"+"items"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if(data.createdAt === undefined){
const err7 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "createdAt"},message:"must have required property '"+"createdAt"+"'"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
for(const key0 in data){
if(!((((((((key0 === "createdAt") || (key0 === "id")) || (key0 === "items")) || (key0 === "mode")) || (key0 === "revision")) || (key0 === "rootId")) || (key0 === "scanId")) || (key0 === "status"))){
const err8 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.createdAt !== undefined){
if(typeof data.createdAt !== "string"){
const err9 = {instancePath:instancePath+"/createdAt",schemaPath:"#/properties/createdAt/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.id !== undefined){
if(typeof data.id !== "string"){
const err10 = {instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
if(data.items !== undefined){
let data2 = data.items;
if(Array.isArray(data2)){
const len0 = data2.length;
for(let i0=0; i0<len0; i0++){
let data3 = data2[i0];
if(data3 && typeof data3 == "object" && !Array.isArray(data3)){
if(data3.id === undefined){
const err11 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "id"},message:"must have required property '"+"id"+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if(data3.fileId === undefined){
const err12 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "fileId"},message:"must have required property '"+"fileId"+"'"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if(data3.source === undefined){
const err13 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "source"},message:"must have required property '"+"source"+"'"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if(data3.target === undefined){
const err14 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if(data3.action === undefined){
const err15 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "action"},message:"must have required property '"+"action"+"'"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
if(data3.selected === undefined){
const err16 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "selected"},message:"must have required property '"+"selected"+"'"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if(data3.origin === undefined){
const err17 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "origin"},message:"must have required property '"+"origin"+"'"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
if(data3.reason === undefined){
const err18 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "reason"},message:"must have required property '"+"reason"+"'"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if(data3.expected === undefined){
const err19 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "expected"},message:"must have required property '"+"expected"+"'"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
for(const key1 in data3){
if(!(func2.call(schema21.properties.items.items.properties, key1))){
const err20 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
}
if(data3.action !== undefined){
let data4 = data3.action;
if(typeof data4 !== "string"){
const err21 = {instancePath:instancePath+"/items/" + i0+"/action",schemaPath:"#/properties/items/items/properties/action/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
if(!((data4 === "move") || (data4 === "noop"))){
const err22 = {instancePath:instancePath+"/items/" + i0+"/action",schemaPath:"#/properties/items/items/properties/action/enum",keyword:"enum",params:{allowedValues: schema21.properties.items.items.properties.action.enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
}
if(data3.expected !== undefined){
let data5 = data3.expected;
if(data5 && typeof data5 == "object" && !Array.isArray(data5)){
if(data5.volumeId === undefined){
const err23 = {instancePath:instancePath+"/items/" + i0+"/expected",schemaPath:"#/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "volumeId"},message:"must have required property '"+"volumeId"+"'"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
if(data5.fileId === undefined){
const err24 = {instancePath:instancePath+"/items/" + i0+"/expected",schemaPath:"#/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "fileId"},message:"must have required property '"+"fileId"+"'"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if(data5.size === undefined){
const err25 = {instancePath:instancePath+"/items/" + i0+"/expected",schemaPath:"#/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "size"},message:"must have required property '"+"size"+"'"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
if(data5.modifiedNs === undefined){
const err26 = {instancePath:instancePath+"/items/" + i0+"/expected",schemaPath:"#/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "modifiedNs"},message:"must have required property '"+"modifiedNs"+"'"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
if(data5.sha256 === undefined){
const err27 = {instancePath:instancePath+"/items/" + i0+"/expected",schemaPath:"#/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "sha256"},message:"must have required property '"+"sha256"+"'"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
for(const key2 in data5){
if(!(((((key2 === "fileId") || (key2 === "modifiedNs")) || (key2 === "sha256")) || (key2 === "size")) || (key2 === "volumeId"))){
const err28 = {instancePath:instancePath+"/items/" + i0+"/expected",schemaPath:"#/properties/items/items/properties/expected/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
}
if(data5.fileId !== undefined){
if(typeof data5.fileId !== "string"){
const err29 = {instancePath:instancePath+"/items/" + i0+"/expected/fileId",schemaPath:"#/properties/items/items/properties/expected/properties/fileId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
}
if(data5.modifiedNs !== undefined){
if(typeof data5.modifiedNs !== "string"){
const err30 = {instancePath:instancePath+"/items/" + i0+"/expected/modifiedNs",schemaPath:"#/properties/items/items/properties/expected/properties/modifiedNs/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
}
if(data5.sha256 !== undefined){
let data8 = data5.sha256;
if((typeof data8 !== "string") && (data8 !== null)){
const err31 = {instancePath:instancePath+"/items/" + i0+"/expected/sha256",schemaPath:"#/properties/items/items/properties/expected/properties/sha256/type",keyword:"type",params:{type: schema21.properties.items.items.properties.expected.properties.sha256.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
}
if(data5.size !== undefined){
if(typeof data5.size !== "string"){
const err32 = {instancePath:instancePath+"/items/" + i0+"/expected/size",schemaPath:"#/properties/items/items/properties/expected/properties/size/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
}
if(data5.volumeId !== undefined){
if(typeof data5.volumeId !== "string"){
const err33 = {instancePath:instancePath+"/items/" + i0+"/expected/volumeId",schemaPath:"#/properties/items/items/properties/expected/properties/volumeId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
}
}
else {
const err34 = {instancePath:instancePath+"/items/" + i0+"/expected",schemaPath:"#/properties/items/items/properties/expected/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
}
if(data3.fileId !== undefined){
if(typeof data3.fileId !== "string"){
const err35 = {instancePath:instancePath+"/items/" + i0+"/fileId",schemaPath:"#/properties/items/items/properties/fileId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
}
if(data3.id !== undefined){
if(typeof data3.id !== "string"){
const err36 = {instancePath:instancePath+"/items/" + i0+"/id",schemaPath:"#/properties/items/items/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
}
if(data3.origin !== undefined){
let data13 = data3.origin;
if(typeof data13 !== "string"){
const err37 = {instancePath:instancePath+"/items/" + i0+"/origin",schemaPath:"#/properties/items/items/properties/origin/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
if(!(((data13 === "rule") || (data13 === "ai")) || (data13 === "user"))){
const err38 = {instancePath:instancePath+"/items/" + i0+"/origin",schemaPath:"#/properties/items/items/properties/origin/enum",keyword:"enum",params:{allowedValues: schema21.properties.items.items.properties.origin.enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
}
if(data3.reason !== undefined){
if(typeof data3.reason !== "string"){
const err39 = {instancePath:instancePath+"/items/" + i0+"/reason",schemaPath:"#/properties/items/items/properties/reason/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
}
if(data3.selected !== undefined){
if(typeof data3.selected !== "boolean"){
const err40 = {instancePath:instancePath+"/items/" + i0+"/selected",schemaPath:"#/properties/items/items/properties/selected/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
}
if(data3.source !== undefined){
let data16 = data3.source;
if(Array.isArray(data16)){
const len1 = data16.length;
for(let i1=0; i1<len1; i1++){
if(typeof data16[i1] !== "string"){
const err41 = {instancePath:instancePath+"/items/" + i0+"/source/" + i1,schemaPath:"#/properties/items/items/properties/source/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
}
}
else {
const err42 = {instancePath:instancePath+"/items/" + i0+"/source",schemaPath:"#/properties/items/items/properties/source/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
}
if(data3.target !== undefined){
let data18 = data3.target;
if(Array.isArray(data18)){
const len2 = data18.length;
for(let i2=0; i2<len2; i2++){
if(typeof data18[i2] !== "string"){
const err43 = {instancePath:instancePath+"/items/" + i0+"/target/" + i2,schemaPath:"#/properties/items/items/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
}
}
else {
const err44 = {instancePath:instancePath+"/items/" + i0+"/target",schemaPath:"#/properties/items/items/properties/target/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
}
}
else {
const err45 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err45];
}
else {
vErrors.push(err45);
}
errors++;
}
}
}
else {
const err46 = {instancePath:instancePath+"/items",schemaPath:"#/properties/items/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err46];
}
else {
vErrors.push(err46);
}
errors++;
}
}
if(data.mode !== undefined){
let data20 = data.mode;
const _errs45 = errors;
let valid9 = false;
let passing0 = null;
const _errs46 = errors;
if(typeof data20 !== "string"){
const err47 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err47];
}
else {
vErrors.push(err47);
}
errors++;
}
if("rules" !== data20){
const err48 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/0/const",keyword:"const",params:{allowedValue: "rules"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err48];
}
else {
vErrors.push(err48);
}
errors++;
}
var _valid0 = _errs46 === errors;
if(_valid0){
valid9 = true;
passing0 = 0;
}
const _errs48 = errors;
if(typeof data20 !== "string"){
const err49 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err49];
}
else {
vErrors.push(err49);
}
errors++;
}
if("aiLocal" !== data20){
const err50 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/1/const",keyword:"const",params:{allowedValue: "aiLocal"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err50];
}
else {
vErrors.push(err50);
}
errors++;
}
var _valid0 = _errs48 === errors;
if(_valid0 && valid9){
valid9 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid9 = true;
passing0 = 1;
}
const _errs50 = errors;
if(typeof data20 !== "string"){
const err51 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err51];
}
else {
vErrors.push(err51);
}
errors++;
}
if("aiCloud" !== data20){
const err52 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf/2/const",keyword:"const",params:{allowedValue: "aiCloud"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err52];
}
else {
vErrors.push(err52);
}
errors++;
}
var _valid0 = _errs50 === errors;
if(_valid0 && valid9){
valid9 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid9 = true;
passing0 = 2;
}
}
}
if(!valid9){
const err53 = {instancePath:instancePath+"/mode",schemaPath:"#/properties/mode/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err53];
}
else {
vErrors.push(err53);
}
errors++;
}
else {
errors = _errs45;
if(vErrors !== null){
if(_errs45){
vErrors.length = _errs45;
}
else {
vErrors = null;
}
}
}
}
if(data.revision !== undefined){
let data21 = data.revision;
if(!(((typeof data21 == "number") && (!(data21 % 1) && !isNaN(data21))) && (isFinite(data21)))){
const err54 = {instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err54];
}
else {
vErrors.push(err54);
}
errors++;
}
if((typeof data21 == "number") && (isFinite(data21))){
if(data21 < 0 || isNaN(data21)){
const err55 = {instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err55];
}
else {
vErrors.push(err55);
}
errors++;
}
}
}
if(data.rootId !== undefined){
if(typeof data.rootId !== "string"){
const err56 = {instancePath:instancePath+"/rootId",schemaPath:"#/properties/rootId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err56];
}
else {
vErrors.push(err56);
}
errors++;
}
}
if(data.scanId !== undefined){
if(typeof data.scanId !== "string"){
const err57 = {instancePath:instancePath+"/scanId",schemaPath:"#/properties/scanId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err57];
}
else {
vErrors.push(err57);
}
errors++;
}
}
if(data.status !== undefined){
let data24 = data.status;
const _errs59 = errors;
let valid10 = false;
let passing1 = null;
const _errs60 = errors;
if(typeof data24 !== "string"){
const err58 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err58];
}
else {
vErrors.push(err58);
}
errors++;
}
if(!((data24 === "draft") || (data24 === "archived"))){
const err59 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema21.properties.status.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err59];
}
else {
vErrors.push(err59);
}
errors++;
}
var _valid1 = _errs60 === errors;
if(_valid1){
valid10 = true;
passing1 = 0;
}
const _errs62 = errors;
if(typeof data24 !== "string"){
const err60 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err60];
}
else {
vErrors.push(err60);
}
errors++;
}
if("validated" !== data24){
const err61 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/1/const",keyword:"const",params:{allowedValue: "validated"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err61];
}
else {
vErrors.push(err61);
}
errors++;
}
var _valid1 = _errs62 === errors;
if(_valid1 && valid10){
valid10 = false;
passing1 = [passing1, 1];
}
else {
if(_valid1){
valid10 = true;
passing1 = 1;
}
const _errs64 = errors;
if(typeof data24 !== "string"){
const err62 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err62];
}
else {
vErrors.push(err62);
}
errors++;
}
if("sealed" !== data24){
const err63 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/2/const",keyword:"const",params:{allowedValue: "sealed"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err63];
}
else {
vErrors.push(err63);
}
errors++;
}
var _valid1 = _errs64 === errors;
if(_valid1 && valid10){
valid10 = false;
passing1 = [passing1, 2];
}
else {
if(_valid1){
valid10 = true;
passing1 = 2;
}
}
}
if(!valid10){
const err64 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf",keyword:"oneOf",params:{passingSchemas: passing1},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err64];
}
else {
vErrors.push(err64);
}
errors++;
}
else {
errors = _errs59;
if(vErrors !== null){
if(_errs59){
vErrors.length = _errs59;
}
else {
vErrors = null;
}
}
}
}
}
else {
const err65 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err65];
}
else {
vErrors.push(err65);
}
errors++;
}
validate20.errors = vErrors;
return errors === 0;
}

export const PlanBuild = validate21;
const schema22 = {"additionalProperties":false,"description":"构建结果。\n\nT05 起它是 `create_plan` 的返回值，因此成为契约类型：\n前端要同时拿到 `plan`（渲染表格）与 `issues`（说明哪些项不可选、为什么）。\n**不派生 `Deserialize`**：它只由后端产出，前端不该有能力提交一个 PlanBuild 回来。","properties":{"issues":{"description":"构建期间发现的问题。`Warning` 表示「这一项不可选，原因如下」。","items":{"additionalProperties":false,"description":"校验或执行过程中的一个问题。","properties":{"code":{"type":"string"},"itemId":{"description":"与具体计划项相关时给出；全局问题为 `None`。","type":["string","null"]},"message":{"type":"string"},"severity":{"description":"校验问题的严重度。","oneOf":[{"enum":["info","warning"],"type":"string"},{"const":"block","description":"阻断项：只要存在，就不能生成可执行计划。","type":"string"}]}},"required":["code","severity","itemId","message"],"type":"object"},"type":"array"},"plan":{"additionalProperties":false,"description":"整理计划。","properties":{"createdAt":{"description":"UTC RFC3339 字符串。","type":"string"},"id":{"type":"string"},"items":{"items":{"additionalProperties":false,"description":"计划中的单项。它已经过确定性规划器处理，带完整源指纹。","properties":{"action":{"description":"单个计划项的动作。v0.1 只有这两种 —— 没有删除、没有覆盖。","enum":["move","noop"],"type":"string"},"expected":{"additionalProperties":false,"description":"生成计划时绑定的源文件指纹。执行前必须重新核对，不一致即停止。","properties":{"fileId":{"description":"Windows 文件身份（卷内唯一）。不等同于路径。","type":"string"},"modifiedNs":{"description":"UTC 纳秒时间戳，十进制字符串。","type":"string"},"sha256":{"description":"生成**可执行计划**时必须为 `Some`。仅用于展示的初扫结果可以为 `None`。","type":["string","null"]},"size":{"description":"十进制字符串，避免 JavaScript 数值精度损失。","type":"string"},"volumeId":{"type":"string"}},"required":["volumeId","fileId","size","modifiedNs","sha256"],"type":"object"},"fileId":{"type":"string"},"id":{"type":"string"},"origin":{"description":"计划项的来源。","enum":["rule","ai","user"],"type":"string"},"reason":{"type":"string"},"selected":{"type":"boolean"},"source":{"items":{"type":"string"},"type":"array"},"target":{"items":{"type":"string"},"type":"array"}},"required":["id","fileId","source","target","action","selected","origin","reason","expected"],"type":"object"},"type":"array"},"mode":{"description":"整理模式。","oneOf":[{"const":"rules","description":"纯规则模式：不联网、不做内容提取，断网与无模型时仍然完整可用。","type":"string"},{"const":"aiLocal","description":"本地模型模式：只允许 loopback 地址。","type":"string"},{"const":"aiCloud","description":"云端模型模式：必须先展示并授权真实待发送载荷。","type":"string"}]},"revision":{"description":"乐观锁版本。任何编辑都会 +1，并使旧校验与旧确认立即失效。","format":"uint32","minimum":0,"type":"integer"},"rootId":{"type":"string"},"scanId":{"type":"string"},"status":{"description":"计划状态。","oneOf":[{"enum":["draft","archived"],"type":"string"},{"const":"validated","description":"已通过校验并取得一次性 validationToken。","type":"string"},{"const":"sealed","description":"已随执行被密封，成为执行输入的唯一来源。","type":"string"}]}},"required":["id","rootId","scanId","revision","mode","status","items","createdAt"],"type":"object"}},"required":["plan","issues"],"type":"object"};

function validate21(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.plan === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "plan"},message:"must have required property '"+"plan"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.issues === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "issues"},message:"must have required property '"+"issues"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
for(const key0 in data){
if(!((key0 === "issues") || (key0 === "plan"))){
const err2 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
if(data.issues !== undefined){
let data0 = data.issues;
if(Array.isArray(data0)){
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
let data1 = data0[i0];
if(data1 && typeof data1 == "object" && !Array.isArray(data1)){
if(data1.code === undefined){
const err3 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "code"},message:"must have required property '"+"code"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data1.severity === undefined){
const err4 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "severity"},message:"must have required property '"+"severity"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data1.itemId === undefined){
const err5 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data1.message === undefined){
const err6 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key1 in data1){
if(!((((key1 === "code") || (key1 === "itemId")) || (key1 === "message")) || (key1 === "severity"))){
const err7 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data1.code !== undefined){
if(typeof data1.code !== "string"){
const err8 = {instancePath:instancePath+"/issues/" + i0+"/code",schemaPath:"#/properties/issues/items/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data1.itemId !== undefined){
let data3 = data1.itemId;
if((typeof data3 !== "string") && (data3 !== null)){
const err9 = {instancePath:instancePath+"/issues/" + i0+"/itemId",schemaPath:"#/properties/issues/items/properties/itemId/type",keyword:"type",params:{type: schema22.properties.issues.items.properties.itemId.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data1.message !== undefined){
if(typeof data1.message !== "string"){
const err10 = {instancePath:instancePath+"/issues/" + i0+"/message",schemaPath:"#/properties/issues/items/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
if(data1.severity !== undefined){
let data5 = data1.severity;
const _errs14 = errors;
let valid4 = false;
let passing0 = null;
const _errs15 = errors;
if(typeof data5 !== "string"){
const err11 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if(!((data5 === "info") || (data5 === "warning"))){
const err12 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema22.properties.issues.items.properties.severity.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
var _valid0 = _errs15 === errors;
if(_valid0){
valid4 = true;
passing0 = 0;
}
const _errs17 = errors;
if(typeof data5 !== "string"){
const err13 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if("block" !== data5){
const err14 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/1/const",keyword:"const",params:{allowedValue: "block"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
var _valid0 = _errs17 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid4 = true;
passing0 = 1;
}
}
if(!valid4){
const err15 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
else {
errors = _errs14;
if(vErrors !== null){
if(_errs14){
vErrors.length = _errs14;
}
else {
vErrors = null;
}
}
}
}
}
else {
const err16 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
}
else {
const err17 = {instancePath:instancePath+"/issues",schemaPath:"#/properties/issues/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
}
if(data.plan !== undefined){
let data6 = data.plan;
if(data6 && typeof data6 == "object" && !Array.isArray(data6)){
if(data6.id === undefined){
const err18 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "id"},message:"must have required property '"+"id"+"'"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if(data6.rootId === undefined){
const err19 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "rootId"},message:"must have required property '"+"rootId"+"'"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
if(data6.scanId === undefined){
const err20 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "scanId"},message:"must have required property '"+"scanId"+"'"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if(data6.revision === undefined){
const err21 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "revision"},message:"must have required property '"+"revision"+"'"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
if(data6.mode === undefined){
const err22 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "mode"},message:"must have required property '"+"mode"+"'"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if(data6.status === undefined){
const err23 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
if(data6.items === undefined){
const err24 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "items"},message:"must have required property '"+"items"+"'"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if(data6.createdAt === undefined){
const err25 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/required",keyword:"required",params:{missingProperty: "createdAt"},message:"must have required property '"+"createdAt"+"'"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
for(const key2 in data6){
if(!((((((((key2 === "createdAt") || (key2 === "id")) || (key2 === "items")) || (key2 === "mode")) || (key2 === "revision")) || (key2 === "rootId")) || (key2 === "scanId")) || (key2 === "status"))){
const err26 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
if(data6.createdAt !== undefined){
if(typeof data6.createdAt !== "string"){
const err27 = {instancePath:instancePath+"/plan/createdAt",schemaPath:"#/properties/plan/properties/createdAt/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
}
if(data6.id !== undefined){
if(typeof data6.id !== "string"){
const err28 = {instancePath:instancePath+"/plan/id",schemaPath:"#/properties/plan/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
}
if(data6.items !== undefined){
let data9 = data6.items;
if(Array.isArray(data9)){
const len1 = data9.length;
for(let i1=0; i1<len1; i1++){
let data10 = data9[i1];
if(data10 && typeof data10 == "object" && !Array.isArray(data10)){
if(data10.id === undefined){
const err29 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "id"},message:"must have required property '"+"id"+"'"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
if(data10.fileId === undefined){
const err30 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "fileId"},message:"must have required property '"+"fileId"+"'"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
if(data10.source === undefined){
const err31 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "source"},message:"must have required property '"+"source"+"'"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
if(data10.target === undefined){
const err32 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
if(data10.action === undefined){
const err33 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "action"},message:"must have required property '"+"action"+"'"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
if(data10.selected === undefined){
const err34 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "selected"},message:"must have required property '"+"selected"+"'"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
if(data10.origin === undefined){
const err35 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "origin"},message:"must have required property '"+"origin"+"'"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
if(data10.reason === undefined){
const err36 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "reason"},message:"must have required property '"+"reason"+"'"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
if(data10.expected === undefined){
const err37 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/required",keyword:"required",params:{missingProperty: "expected"},message:"must have required property '"+"expected"+"'"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
for(const key3 in data10){
if(!(func2.call(schema22.properties.plan.properties.items.items.properties, key3))){
const err38 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
}
if(data10.action !== undefined){
let data11 = data10.action;
if(typeof data11 !== "string"){
const err39 = {instancePath:instancePath+"/plan/items/" + i1+"/action",schemaPath:"#/properties/plan/properties/items/items/properties/action/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
if(!((data11 === "move") || (data11 === "noop"))){
const err40 = {instancePath:instancePath+"/plan/items/" + i1+"/action",schemaPath:"#/properties/plan/properties/items/items/properties/action/enum",keyword:"enum",params:{allowedValues: schema22.properties.plan.properties.items.items.properties.action.enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
}
if(data10.expected !== undefined){
let data12 = data10.expected;
if(data12 && typeof data12 == "object" && !Array.isArray(data12)){
if(data12.volumeId === undefined){
const err41 = {instancePath:instancePath+"/plan/items/" + i1+"/expected",schemaPath:"#/properties/plan/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "volumeId"},message:"must have required property '"+"volumeId"+"'"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
if(data12.fileId === undefined){
const err42 = {instancePath:instancePath+"/plan/items/" + i1+"/expected",schemaPath:"#/properties/plan/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "fileId"},message:"must have required property '"+"fileId"+"'"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
if(data12.size === undefined){
const err43 = {instancePath:instancePath+"/plan/items/" + i1+"/expected",schemaPath:"#/properties/plan/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "size"},message:"must have required property '"+"size"+"'"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
if(data12.modifiedNs === undefined){
const err44 = {instancePath:instancePath+"/plan/items/" + i1+"/expected",schemaPath:"#/properties/plan/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "modifiedNs"},message:"must have required property '"+"modifiedNs"+"'"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
if(data12.sha256 === undefined){
const err45 = {instancePath:instancePath+"/plan/items/" + i1+"/expected",schemaPath:"#/properties/plan/properties/items/items/properties/expected/required",keyword:"required",params:{missingProperty: "sha256"},message:"must have required property '"+"sha256"+"'"};
if(vErrors === null){
vErrors = [err45];
}
else {
vErrors.push(err45);
}
errors++;
}
for(const key4 in data12){
if(!(((((key4 === "fileId") || (key4 === "modifiedNs")) || (key4 === "sha256")) || (key4 === "size")) || (key4 === "volumeId"))){
const err46 = {instancePath:instancePath+"/plan/items/" + i1+"/expected",schemaPath:"#/properties/plan/properties/items/items/properties/expected/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key4},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err46];
}
else {
vErrors.push(err46);
}
errors++;
}
}
if(data12.fileId !== undefined){
if(typeof data12.fileId !== "string"){
const err47 = {instancePath:instancePath+"/plan/items/" + i1+"/expected/fileId",schemaPath:"#/properties/plan/properties/items/items/properties/expected/properties/fileId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err47];
}
else {
vErrors.push(err47);
}
errors++;
}
}
if(data12.modifiedNs !== undefined){
if(typeof data12.modifiedNs !== "string"){
const err48 = {instancePath:instancePath+"/plan/items/" + i1+"/expected/modifiedNs",schemaPath:"#/properties/plan/properties/items/items/properties/expected/properties/modifiedNs/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err48];
}
else {
vErrors.push(err48);
}
errors++;
}
}
if(data12.sha256 !== undefined){
let data15 = data12.sha256;
if((typeof data15 !== "string") && (data15 !== null)){
const err49 = {instancePath:instancePath+"/plan/items/" + i1+"/expected/sha256",schemaPath:"#/properties/plan/properties/items/items/properties/expected/properties/sha256/type",keyword:"type",params:{type: schema22.properties.plan.properties.items.items.properties.expected.properties.sha256.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err49];
}
else {
vErrors.push(err49);
}
errors++;
}
}
if(data12.size !== undefined){
if(typeof data12.size !== "string"){
const err50 = {instancePath:instancePath+"/plan/items/" + i1+"/expected/size",schemaPath:"#/properties/plan/properties/items/items/properties/expected/properties/size/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err50];
}
else {
vErrors.push(err50);
}
errors++;
}
}
if(data12.volumeId !== undefined){
if(typeof data12.volumeId !== "string"){
const err51 = {instancePath:instancePath+"/plan/items/" + i1+"/expected/volumeId",schemaPath:"#/properties/plan/properties/items/items/properties/expected/properties/volumeId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err51];
}
else {
vErrors.push(err51);
}
errors++;
}
}
}
else {
const err52 = {instancePath:instancePath+"/plan/items/" + i1+"/expected",schemaPath:"#/properties/plan/properties/items/items/properties/expected/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err52];
}
else {
vErrors.push(err52);
}
errors++;
}
}
if(data10.fileId !== undefined){
if(typeof data10.fileId !== "string"){
const err53 = {instancePath:instancePath+"/plan/items/" + i1+"/fileId",schemaPath:"#/properties/plan/properties/items/items/properties/fileId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err53];
}
else {
vErrors.push(err53);
}
errors++;
}
}
if(data10.id !== undefined){
if(typeof data10.id !== "string"){
const err54 = {instancePath:instancePath+"/plan/items/" + i1+"/id",schemaPath:"#/properties/plan/properties/items/items/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err54];
}
else {
vErrors.push(err54);
}
errors++;
}
}
if(data10.origin !== undefined){
let data20 = data10.origin;
if(typeof data20 !== "string"){
const err55 = {instancePath:instancePath+"/plan/items/" + i1+"/origin",schemaPath:"#/properties/plan/properties/items/items/properties/origin/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err55];
}
else {
vErrors.push(err55);
}
errors++;
}
if(!(((data20 === "rule") || (data20 === "ai")) || (data20 === "user"))){
const err56 = {instancePath:instancePath+"/plan/items/" + i1+"/origin",schemaPath:"#/properties/plan/properties/items/items/properties/origin/enum",keyword:"enum",params:{allowedValues: schema22.properties.plan.properties.items.items.properties.origin.enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err56];
}
else {
vErrors.push(err56);
}
errors++;
}
}
if(data10.reason !== undefined){
if(typeof data10.reason !== "string"){
const err57 = {instancePath:instancePath+"/plan/items/" + i1+"/reason",schemaPath:"#/properties/plan/properties/items/items/properties/reason/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err57];
}
else {
vErrors.push(err57);
}
errors++;
}
}
if(data10.selected !== undefined){
if(typeof data10.selected !== "boolean"){
const err58 = {instancePath:instancePath+"/plan/items/" + i1+"/selected",schemaPath:"#/properties/plan/properties/items/items/properties/selected/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err58];
}
else {
vErrors.push(err58);
}
errors++;
}
}
if(data10.source !== undefined){
let data23 = data10.source;
if(Array.isArray(data23)){
const len2 = data23.length;
for(let i2=0; i2<len2; i2++){
if(typeof data23[i2] !== "string"){
const err59 = {instancePath:instancePath+"/plan/items/" + i1+"/source/" + i2,schemaPath:"#/properties/plan/properties/items/items/properties/source/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err59];
}
else {
vErrors.push(err59);
}
errors++;
}
}
}
else {
const err60 = {instancePath:instancePath+"/plan/items/" + i1+"/source",schemaPath:"#/properties/plan/properties/items/items/properties/source/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err60];
}
else {
vErrors.push(err60);
}
errors++;
}
}
if(data10.target !== undefined){
let data25 = data10.target;
if(Array.isArray(data25)){
const len3 = data25.length;
for(let i3=0; i3<len3; i3++){
if(typeof data25[i3] !== "string"){
const err61 = {instancePath:instancePath+"/plan/items/" + i1+"/target/" + i3,schemaPath:"#/properties/plan/properties/items/items/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err61];
}
else {
vErrors.push(err61);
}
errors++;
}
}
}
else {
const err62 = {instancePath:instancePath+"/plan/items/" + i1+"/target",schemaPath:"#/properties/plan/properties/items/items/properties/target/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err62];
}
else {
vErrors.push(err62);
}
errors++;
}
}
}
else {
const err63 = {instancePath:instancePath+"/plan/items/" + i1,schemaPath:"#/properties/plan/properties/items/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err63];
}
else {
vErrors.push(err63);
}
errors++;
}
}
}
else {
const err64 = {instancePath:instancePath+"/plan/items",schemaPath:"#/properties/plan/properties/items/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err64];
}
else {
vErrors.push(err64);
}
errors++;
}
}
if(data6.mode !== undefined){
let data27 = data6.mode;
const _errs65 = errors;
let valid14 = false;
let passing1 = null;
const _errs66 = errors;
if(typeof data27 !== "string"){
const err65 = {instancePath:instancePath+"/plan/mode",schemaPath:"#/properties/plan/properties/mode/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err65];
}
else {
vErrors.push(err65);
}
errors++;
}
if("rules" !== data27){
const err66 = {instancePath:instancePath+"/plan/mode",schemaPath:"#/properties/plan/properties/mode/oneOf/0/const",keyword:"const",params:{allowedValue: "rules"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err66];
}
else {
vErrors.push(err66);
}
errors++;
}
var _valid1 = _errs66 === errors;
if(_valid1){
valid14 = true;
passing1 = 0;
}
const _errs68 = errors;
if(typeof data27 !== "string"){
const err67 = {instancePath:instancePath+"/plan/mode",schemaPath:"#/properties/plan/properties/mode/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err67];
}
else {
vErrors.push(err67);
}
errors++;
}
if("aiLocal" !== data27){
const err68 = {instancePath:instancePath+"/plan/mode",schemaPath:"#/properties/plan/properties/mode/oneOf/1/const",keyword:"const",params:{allowedValue: "aiLocal"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err68];
}
else {
vErrors.push(err68);
}
errors++;
}
var _valid1 = _errs68 === errors;
if(_valid1 && valid14){
valid14 = false;
passing1 = [passing1, 1];
}
else {
if(_valid1){
valid14 = true;
passing1 = 1;
}
const _errs70 = errors;
if(typeof data27 !== "string"){
const err69 = {instancePath:instancePath+"/plan/mode",schemaPath:"#/properties/plan/properties/mode/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err69];
}
else {
vErrors.push(err69);
}
errors++;
}
if("aiCloud" !== data27){
const err70 = {instancePath:instancePath+"/plan/mode",schemaPath:"#/properties/plan/properties/mode/oneOf/2/const",keyword:"const",params:{allowedValue: "aiCloud"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err70];
}
else {
vErrors.push(err70);
}
errors++;
}
var _valid1 = _errs70 === errors;
if(_valid1 && valid14){
valid14 = false;
passing1 = [passing1, 2];
}
else {
if(_valid1){
valid14 = true;
passing1 = 2;
}
}
}
if(!valid14){
const err71 = {instancePath:instancePath+"/plan/mode",schemaPath:"#/properties/plan/properties/mode/oneOf",keyword:"oneOf",params:{passingSchemas: passing1},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err71];
}
else {
vErrors.push(err71);
}
errors++;
}
else {
errors = _errs65;
if(vErrors !== null){
if(_errs65){
vErrors.length = _errs65;
}
else {
vErrors = null;
}
}
}
}
if(data6.revision !== undefined){
let data28 = data6.revision;
if(!(((typeof data28 == "number") && (!(data28 % 1) && !isNaN(data28))) && (isFinite(data28)))){
const err72 = {instancePath:instancePath+"/plan/revision",schemaPath:"#/properties/plan/properties/revision/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err72];
}
else {
vErrors.push(err72);
}
errors++;
}
if((typeof data28 == "number") && (isFinite(data28))){
if(data28 < 0 || isNaN(data28)){
const err73 = {instancePath:instancePath+"/plan/revision",schemaPath:"#/properties/plan/properties/revision/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err73];
}
else {
vErrors.push(err73);
}
errors++;
}
}
}
if(data6.rootId !== undefined){
if(typeof data6.rootId !== "string"){
const err74 = {instancePath:instancePath+"/plan/rootId",schemaPath:"#/properties/plan/properties/rootId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err74];
}
else {
vErrors.push(err74);
}
errors++;
}
}
if(data6.scanId !== undefined){
if(typeof data6.scanId !== "string"){
const err75 = {instancePath:instancePath+"/plan/scanId",schemaPath:"#/properties/plan/properties/scanId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err75];
}
else {
vErrors.push(err75);
}
errors++;
}
}
if(data6.status !== undefined){
let data31 = data6.status;
const _errs79 = errors;
let valid15 = false;
let passing2 = null;
const _errs80 = errors;
if(typeof data31 !== "string"){
const err76 = {instancePath:instancePath+"/plan/status",schemaPath:"#/properties/plan/properties/status/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err76];
}
else {
vErrors.push(err76);
}
errors++;
}
if(!((data31 === "draft") || (data31 === "archived"))){
const err77 = {instancePath:instancePath+"/plan/status",schemaPath:"#/properties/plan/properties/status/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema22.properties.plan.properties.status.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err77];
}
else {
vErrors.push(err77);
}
errors++;
}
var _valid2 = _errs80 === errors;
if(_valid2){
valid15 = true;
passing2 = 0;
}
const _errs82 = errors;
if(typeof data31 !== "string"){
const err78 = {instancePath:instancePath+"/plan/status",schemaPath:"#/properties/plan/properties/status/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err78];
}
else {
vErrors.push(err78);
}
errors++;
}
if("validated" !== data31){
const err79 = {instancePath:instancePath+"/plan/status",schemaPath:"#/properties/plan/properties/status/oneOf/1/const",keyword:"const",params:{allowedValue: "validated"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err79];
}
else {
vErrors.push(err79);
}
errors++;
}
var _valid2 = _errs82 === errors;
if(_valid2 && valid15){
valid15 = false;
passing2 = [passing2, 1];
}
else {
if(_valid2){
valid15 = true;
passing2 = 1;
}
const _errs84 = errors;
if(typeof data31 !== "string"){
const err80 = {instancePath:instancePath+"/plan/status",schemaPath:"#/properties/plan/properties/status/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err80];
}
else {
vErrors.push(err80);
}
errors++;
}
if("sealed" !== data31){
const err81 = {instancePath:instancePath+"/plan/status",schemaPath:"#/properties/plan/properties/status/oneOf/2/const",keyword:"const",params:{allowedValue: "sealed"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err81];
}
else {
vErrors.push(err81);
}
errors++;
}
var _valid2 = _errs84 === errors;
if(_valid2 && valid15){
valid15 = false;
passing2 = [passing2, 2];
}
else {
if(_valid2){
valid15 = true;
passing2 = 2;
}
}
}
if(!valid15){
const err82 = {instancePath:instancePath+"/plan/status",schemaPath:"#/properties/plan/properties/status/oneOf",keyword:"oneOf",params:{passingSchemas: passing2},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err82];
}
else {
vErrors.push(err82);
}
errors++;
}
else {
errors = _errs79;
if(vErrors !== null){
if(_errs79){
vErrors.length = _errs79;
}
else {
vErrors = null;
}
}
}
}
}
else {
const err83 = {instancePath:instancePath+"/plan",schemaPath:"#/properties/plan/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err83];
}
else {
vErrors.push(err83);
}
errors++;
}
}
}
else {
const err84 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err84];
}
else {
vErrors.push(err84);
}
errors++;
}
validate21.errors = vErrors;
return errors === 0;
}

export const PlanItemEdit = validate22;
const schema23 = {"additionalProperties":false,"description":"一次计划编辑。\n\n规格 5.2：`update_plan` 接受一组这样的编辑，配合 `expectedRevision` 做乐观锁。\n\n只暴露**可编辑的两件事**（勾选与目标路径），其余字段（rootId / scanId / mode /\ncreatedAt / fileId / expected 指纹）一律不接受前端提交——\n让前端能改指纹等于让它可以绕过执行前的核对。\n\n`Option<T>` 在这里表示「必填可空」：字段必须在，值为 `null` 表示不修改。\n**不要给这两个字段加 `#[schemars(required)]`** —— 那会把 `Option<T>` 当成 `T`，\n把 `null` 从契约里吃掉，前端传 `null` 就会被自己的运行时校验拒绝。","properties":{"itemId":{"type":"string"},"selected":{"description":"不修改勾选状态时传 `null`。","type":["boolean","null"]},"target":{"description":"新的目标相对路径（组件数组）。不修改时传 `null`。","items":{"type":"string"},"type":["array","null"]}},"required":["itemId","selected","target"],"type":"object"};

function validate22(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.itemId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.selected === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "selected"},message:"must have required property '"+"selected"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.target === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
for(const key0 in data){
if(!(((key0 === "itemId") || (key0 === "selected")) || (key0 === "target"))){
const err3 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.itemId !== undefined){
if(typeof data.itemId !== "string"){
const err4 = {instancePath:instancePath+"/itemId",schemaPath:"#/properties/itemId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
if(data.selected !== undefined){
let data1 = data.selected;
if((typeof data1 !== "boolean") && (data1 !== null)){
const err5 = {instancePath:instancePath+"/selected",schemaPath:"#/properties/selected/type",keyword:"type",params:{type: schema23.properties.selected.type},message:"must be boolean,null"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
if(data.target !== undefined){
let data2 = data.target;
if((!(Array.isArray(data2))) && (data2 !== null)){
const err6 = {instancePath:instancePath+"/target",schemaPath:"#/properties/target/type",keyword:"type",params:{type: schema23.properties.target.type},message:"must be array,null"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if(Array.isArray(data2)){
const len0 = data2.length;
for(let i0=0; i0<len0; i0++){
if(typeof data2[i0] !== "string"){
const err7 = {instancePath:instancePath+"/target/" + i0,schemaPath:"#/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
}
}
}
else {
const err8 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
validate22.errors = vErrors;
return errors === 0;
}

export const ProviderKind = validate23;
const schema24 = {"description":"提供商类型。\n\n只有两类，因为它们对应**两条不同的安全规则**：\n本地的端点只允许 loopback，云端的只接受 HTTPS。","oneOf":[{"const":"local","description":"本机跑的 Ollama 之类。","type":"string"},{"const":"compatible","description":"OpenAI 兼容的云端接口。","type":"string"}]};

function validate23(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("local" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "local"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("compatible" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "compatible"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate23.errors = vErrors;
return errors === 0;
}

export const ProviderProbe = validate24;
const schema25 = {"additionalProperties":false,"description":"连通性探测的结果（`test_provider`）。","properties":{"message":{"description":"给用户的一句话，含下一步动作。","type":"string"},"model":{"description":"服务端回显的模型名。用户据此确认「我连的确实是我以为的那个模型」。","type":"string"},"structuredOutput":{"description":"这次探测是否拿到了**符合结构**的建议数组。\n\n规格 6.4：「提供商的结构化输出能力需探测；即使服务端宣称保证 JSON，\n客户端仍必须校验。」这个布尔量就是那次校验的结果——\n它是**探测出来的**，不是读某个配置项读来的。","type":"boolean"}},"required":["model","structuredOutput","message"],"type":"object"};

function validate24(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.model === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "model"},message:"must have required property '"+"model"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.structuredOutput === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "structuredOutput"},message:"must have required property '"+"structuredOutput"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.message === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
for(const key0 in data){
if(!(((key0 === "message") || (key0 === "model")) || (key0 === "structuredOutput"))){
const err3 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.message !== undefined){
if(typeof data.message !== "string"){
const err4 = {instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
if(data.model !== undefined){
if(typeof data.model !== "string"){
const err5 = {instancePath:instancePath+"/model",schemaPath:"#/properties/model/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
if(data.structuredOutput !== undefined){
if(typeof data.structuredOutput !== "boolean"){
const err6 = {instancePath:instancePath+"/structuredOutput",schemaPath:"#/properties/structuredOutput/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
}
else {
const err7 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
validate24.errors = vErrors;
return errors === 0;
}

export const ProviderSummary = validate25;
const schema26 = {"additionalProperties":false,"description":"给设置页看的提供商摘要。\n\n比 [`ProviderConfig`] 多一个 [`Self::has_credential`]：界面要显示\n「已保存密钥」，但**不能**显示密钥本身。这一个布尔量就是那个信息，\n而它是单向的——拿到 `true` 也读不出任何内容。","properties":{"endpoint":{"type":"string"},"hasCredential":{"type":"boolean"},"id":{"type":"string"},"kind":{"description":"提供商类型。\n\n只有两类，因为它们对应**两条不同的安全规则**：\n本地的端点只允许 loopback，云端的只接受 HTTPS。","oneOf":[{"const":"local","description":"本机跑的 Ollama 之类。","type":"string"},{"const":"compatible","description":"OpenAI 兼容的云端接口。","type":"string"}]},"model":{"type":"string"}},"required":["id","kind","endpoint","model","hasCredential"],"type":"object"};

function validate25(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.id === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "id"},message:"must have required property '"+"id"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.kind === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "kind"},message:"must have required property '"+"kind"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.endpoint === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "endpoint"},message:"must have required property '"+"endpoint"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.model === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "model"},message:"must have required property '"+"model"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.hasCredential === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "hasCredential"},message:"must have required property '"+"hasCredential"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
for(const key0 in data){
if(!(((((key0 === "endpoint") || (key0 === "hasCredential")) || (key0 === "id")) || (key0 === "kind")) || (key0 === "model"))){
const err5 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
if(data.endpoint !== undefined){
if(typeof data.endpoint !== "string"){
const err6 = {instancePath:instancePath+"/endpoint",schemaPath:"#/properties/endpoint/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
if(data.hasCredential !== undefined){
if(typeof data.hasCredential !== "boolean"){
const err7 = {instancePath:instancePath+"/hasCredential",schemaPath:"#/properties/hasCredential/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.id !== undefined){
if(typeof data.id !== "string"){
const err8 = {instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.kind !== undefined){
let data3 = data.kind;
const _errs9 = errors;
let valid1 = false;
let passing0 = null;
const _errs10 = errors;
if(typeof data3 !== "string"){
const err9 = {instancePath:instancePath+"/kind",schemaPath:"#/properties/kind/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
if("local" !== data3){
const err10 = {instancePath:instancePath+"/kind",schemaPath:"#/properties/kind/oneOf/0/const",keyword:"const",params:{allowedValue: "local"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
var _valid0 = _errs10 === errors;
if(_valid0){
valid1 = true;
passing0 = 0;
}
const _errs12 = errors;
if(typeof data3 !== "string"){
const err11 = {instancePath:instancePath+"/kind",schemaPath:"#/properties/kind/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if("compatible" !== data3){
const err12 = {instancePath:instancePath+"/kind",schemaPath:"#/properties/kind/oneOf/1/const",keyword:"const",params:{allowedValue: "compatible"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
var _valid0 = _errs12 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid1 = true;
passing0 = 1;
}
}
if(!valid1){
const err13 = {instancePath:instancePath+"/kind",schemaPath:"#/properties/kind/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
else {
errors = _errs9;
if(vErrors !== null){
if(_errs9){
vErrors.length = _errs9;
}
else {
vErrors = null;
}
}
}
}
if(data.model !== undefined){
if(typeof data.model !== "string"){
const err14 = {instancePath:instancePath+"/model",schemaPath:"#/properties/model/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
}
}
else {
const err15 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
validate25.errors = vErrors;
return errors === 0;
}

export const RecoveryItem = validate26;
const schema27 = {"additionalProperties":false,"description":"一个未决项的核对结果（规格 8.3）。","properties":{"itemId":{"type":"string"},"message":{"description":"为什么是这个判定。这句话会直接显示在界面上，写的是「用户能据以行动」的内容。","type":"string"},"operationId":{"type":"string"},"resolution":{"description":"未决事实的人工处置（规格 8.2）。\n\n**与 `OpStatus` 是两个维度，不要合并**：\n`status` 记录「磁盘上现在是什么样」，`resolution` 记录「用户对这件事做了什么决定」。\n把「用户确认保留现状」写成 `status = skipped` 之类的值，等于把\n「这一项其实没做成」这个事实抹掉——审计记录要能同时回答这两个问题。","oneOf":[{"const":"open","description":"尚未处置。","type":"string"},{"const":"acknowledged","description":"用户已核对并明确接受「保留现状」，附有理由。","type":"string"}]},"source":{"items":{"type":"string"},"type":"array"},"status":{"description":"单个文件操作的状态（规格 8.2）。\n\n迁移规则见 [`crate::domain::states`]。","oneOf":[{"enum":["pending","applied","failed"],"type":"string"},{"const":"prepared","description":"意图记录已持久化，但尚未确认文件是否被改动。","type":"string"},{"const":"skipped","description":"因取消而未派发。","type":"string"},{"const":"ambiguous","description":"无法确定是否已改动 —— 不能猜，必须人工核对。","type":"string"}]},"target":{"items":{"type":"string"},"type":"array"}},"required":["operationId","itemId","source","target","status","resolution","message"],"type":"object"};

function validate26(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.operationId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "operationId"},message:"must have required property '"+"operationId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.itemId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.source === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "source"},message:"must have required property '"+"source"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.target === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.status === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.resolution === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "resolution"},message:"must have required property '"+"resolution"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.message === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key0 in data){
if(!(((((((key0 === "itemId") || (key0 === "message")) || (key0 === "operationId")) || (key0 === "resolution")) || (key0 === "source")) || (key0 === "status")) || (key0 === "target"))){
const err7 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.itemId !== undefined){
if(typeof data.itemId !== "string"){
const err8 = {instancePath:instancePath+"/itemId",schemaPath:"#/properties/itemId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.message !== undefined){
if(typeof data.message !== "string"){
const err9 = {instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.operationId !== undefined){
if(typeof data.operationId !== "string"){
const err10 = {instancePath:instancePath+"/operationId",schemaPath:"#/properties/operationId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
if(data.resolution !== undefined){
let data3 = data.resolution;
const _errs9 = errors;
let valid1 = false;
let passing0 = null;
const _errs10 = errors;
if(typeof data3 !== "string"){
const err11 = {instancePath:instancePath+"/resolution",schemaPath:"#/properties/resolution/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if("open" !== data3){
const err12 = {instancePath:instancePath+"/resolution",schemaPath:"#/properties/resolution/oneOf/0/const",keyword:"const",params:{allowedValue: "open"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
var _valid0 = _errs10 === errors;
if(_valid0){
valid1 = true;
passing0 = 0;
}
const _errs12 = errors;
if(typeof data3 !== "string"){
const err13 = {instancePath:instancePath+"/resolution",schemaPath:"#/properties/resolution/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if("acknowledged" !== data3){
const err14 = {instancePath:instancePath+"/resolution",schemaPath:"#/properties/resolution/oneOf/1/const",keyword:"const",params:{allowedValue: "acknowledged"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
var _valid0 = _errs12 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid1 = true;
passing0 = 1;
}
}
if(!valid1){
const err15 = {instancePath:instancePath+"/resolution",schemaPath:"#/properties/resolution/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
else {
errors = _errs9;
if(vErrors !== null){
if(_errs9){
vErrors.length = _errs9;
}
else {
vErrors = null;
}
}
}
}
if(data.source !== undefined){
let data4 = data.source;
if(Array.isArray(data4)){
const len0 = data4.length;
for(let i0=0; i0<len0; i0++){
if(typeof data4[i0] !== "string"){
const err16 = {instancePath:instancePath+"/source/" + i0,schemaPath:"#/properties/source/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
}
else {
const err17 = {instancePath:instancePath+"/source",schemaPath:"#/properties/source/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
}
if(data.status !== undefined){
let data6 = data.status;
const _errs19 = errors;
let valid4 = false;
let passing1 = null;
const _errs20 = errors;
if(typeof data6 !== "string"){
const err18 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if(!(((data6 === "pending") || (data6 === "applied")) || (data6 === "failed"))){
const err19 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema27.properties.status.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
var _valid1 = _errs20 === errors;
if(_valid1){
valid4 = true;
passing1 = 0;
}
const _errs22 = errors;
if(typeof data6 !== "string"){
const err20 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if("prepared" !== data6){
const err21 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/1/const",keyword:"const",params:{allowedValue: "prepared"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
var _valid1 = _errs22 === errors;
if(_valid1 && valid4){
valid4 = false;
passing1 = [passing1, 1];
}
else {
if(_valid1){
valid4 = true;
passing1 = 1;
}
const _errs24 = errors;
if(typeof data6 !== "string"){
const err22 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if("skipped" !== data6){
const err23 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/2/const",keyword:"const",params:{allowedValue: "skipped"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
var _valid1 = _errs24 === errors;
if(_valid1 && valid4){
valid4 = false;
passing1 = [passing1, 2];
}
else {
if(_valid1){
valid4 = true;
passing1 = 2;
}
const _errs26 = errors;
if(typeof data6 !== "string"){
const err24 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if("ambiguous" !== data6){
const err25 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/3/const",keyword:"const",params:{allowedValue: "ambiguous"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
var _valid1 = _errs26 === errors;
if(_valid1 && valid4){
valid4 = false;
passing1 = [passing1, 3];
}
else {
if(_valid1){
valid4 = true;
passing1 = 3;
}
}
}
}
if(!valid4){
const err26 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf",keyword:"oneOf",params:{passingSchemas: passing1},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
else {
errors = _errs19;
if(vErrors !== null){
if(_errs19){
vErrors.length = _errs19;
}
else {
vErrors = null;
}
}
}
}
if(data.target !== undefined){
let data7 = data.target;
if(Array.isArray(data7)){
const len1 = data7.length;
for(let i1=0; i1<len1; i1++){
if(typeof data7[i1] !== "string"){
const err27 = {instancePath:instancePath+"/target/" + i1,schemaPath:"#/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
}
}
else {
const err28 = {instancePath:instancePath+"/target",schemaPath:"#/properties/target/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
}
}
else {
const err29 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
validate26.errors = vErrors;
return errors === 0;
}

export const RecoveryReport = validate27;
const schema28 = {"additionalProperties":false,"description":"恢复核对的结果（规格 8.3）。","properties":{"blocksNewRuns":{"description":"仍有未决项没被处置时为 true —— 此时后端**禁止**新的执行任务。","type":"boolean"},"counts":{"additionalProperties":false,"description":"执行结果计数。","properties":{"ambiguous":{"description":"判定不出来、需要人工核对的项数（规格 8.3）。\n\n**单独计数而不是并进 `pending`**：界面上「还没轮到他」和\n「做了但说不清」需要不同的措辞和不同的动作，混在一起用户无从下手。","format":"uint32","minimum":0,"type":"integer"},"applied":{"format":"uint32","minimum":0,"type":"integer"},"failed":{"format":"uint32","minimum":0,"type":"integer"},"pending":{"format":"uint32","minimum":0,"type":"integer"},"skipped":{"format":"uint32","minimum":0,"type":"integer"}},"required":["applied","failed","skipped","pending","ambiguous"],"type":"object"},"items":{"items":{"additionalProperties":false,"description":"一个未决项的核对结果（规格 8.3）。","properties":{"itemId":{"type":"string"},"message":{"description":"为什么是这个判定。这句话会直接显示在界面上，写的是「用户能据以行动」的内容。","type":"string"},"operationId":{"type":"string"},"resolution":{"description":"未决事实的人工处置（规格 8.2）。\n\n**与 `OpStatus` 是两个维度，不要合并**：\n`status` 记录「磁盘上现在是什么样」，`resolution` 记录「用户对这件事做了什么决定」。\n把「用户确认保留现状」写成 `status = skipped` 之类的值，等于把\n「这一项其实没做成」这个事实抹掉——审计记录要能同时回答这两个问题。","oneOf":[{"const":"open","description":"尚未处置。","type":"string"},{"const":"acknowledged","description":"用户已核对并明确接受「保留现状」，附有理由。","type":"string"}]},"source":{"items":{"type":"string"},"type":"array"},"status":{"description":"单个文件操作的状态（规格 8.2）。\n\n迁移规则见 [`crate::domain::states`]。","oneOf":[{"enum":["pending","applied","failed"],"type":"string"},{"const":"prepared","description":"意图记录已持久化，但尚未确认文件是否被改动。","type":"string"},{"const":"skipped","description":"因取消而未派发。","type":"string"},{"const":"ambiguous","description":"无法确定是否已改动 —— 不能猜，必须人工核对。","type":"string"}]},"target":{"items":{"type":"string"},"type":"array"}},"required":["operationId","itemId","source","target","status","resolution","message"],"type":"object"},"type":"array"},"planId":{"type":"string"},"rootAuthorized":{"description":"该 run 的根目录在当前会话里是否仍被授权。\n\n未授权的根换不回路径，核对根本无从下手。这时要**如实说明**，\n而不是回报一份「没有未决项」的空报告——那会让界面显示一切正常。","type":"boolean"},"runId":{"type":"string"},"stateDigest":{"description":"**确认必须带上它**：两次读取之间只要有任意一项的状态或处置变了，\n摘要就会变，后端据此拒绝一份过期的确认。","type":"string"},"status":{"description":"一次执行（含撤销执行）的状态。","enum":["queued","running","completed","partial","failed","cancelled","recoveryRequired"],"type":"string"}},"required":["runId","planId","status","stateDigest","items","counts","blocksNewRuns","rootAuthorized"],"type":"object"};

function validate27(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.runId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "runId"},message:"must have required property '"+"runId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.planId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "planId"},message:"must have required property '"+"planId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.status === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.stateDigest === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "stateDigest"},message:"must have required property '"+"stateDigest"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.items === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "items"},message:"must have required property '"+"items"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.counts === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "counts"},message:"must have required property '"+"counts"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.blocksNewRuns === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "blocksNewRuns"},message:"must have required property '"+"blocksNewRuns"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if(data.rootAuthorized === undefined){
const err7 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "rootAuthorized"},message:"must have required property '"+"rootAuthorized"+"'"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
for(const key0 in data){
if(!((((((((key0 === "blocksNewRuns") || (key0 === "counts")) || (key0 === "items")) || (key0 === "planId")) || (key0 === "rootAuthorized")) || (key0 === "runId")) || (key0 === "stateDigest")) || (key0 === "status"))){
const err8 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.blocksNewRuns !== undefined){
if(typeof data.blocksNewRuns !== "boolean"){
const err9 = {instancePath:instancePath+"/blocksNewRuns",schemaPath:"#/properties/blocksNewRuns/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.counts !== undefined){
let data1 = data.counts;
if(data1 && typeof data1 == "object" && !Array.isArray(data1)){
if(data1.applied === undefined){
const err10 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "applied"},message:"must have required property '"+"applied"+"'"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if(data1.failed === undefined){
const err11 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "failed"},message:"must have required property '"+"failed"+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if(data1.skipped === undefined){
const err12 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "skipped"},message:"must have required property '"+"skipped"+"'"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if(data1.pending === undefined){
const err13 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "pending"},message:"must have required property '"+"pending"+"'"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if(data1.ambiguous === undefined){
const err14 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "ambiguous"},message:"must have required property '"+"ambiguous"+"'"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
for(const key1 in data1){
if(!(((((key1 === "ambiguous") || (key1 === "applied")) || (key1 === "failed")) || (key1 === "pending")) || (key1 === "skipped"))){
const err15 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
}
if(data1.ambiguous !== undefined){
let data2 = data1.ambiguous;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
const err16 = {instancePath:instancePath+"/counts/ambiguous",schemaPath:"#/properties/counts/properties/ambiguous/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
const err17 = {instancePath:instancePath+"/counts/ambiguous",schemaPath:"#/properties/counts/properties/ambiguous/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
}
}
if(data1.applied !== undefined){
let data3 = data1.applied;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
const err18 = {instancePath:instancePath+"/counts/applied",schemaPath:"#/properties/counts/properties/applied/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
const err19 = {instancePath:instancePath+"/counts/applied",schemaPath:"#/properties/counts/properties/applied/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
}
}
if(data1.failed !== undefined){
let data4 = data1.failed;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
const err20 = {instancePath:instancePath+"/counts/failed",schemaPath:"#/properties/counts/properties/failed/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
const err21 = {instancePath:instancePath+"/counts/failed",schemaPath:"#/properties/counts/properties/failed/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
}
if(data1.pending !== undefined){
let data5 = data1.pending;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
const err22 = {instancePath:instancePath+"/counts/pending",schemaPath:"#/properties/counts/properties/pending/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
const err23 = {instancePath:instancePath+"/counts/pending",schemaPath:"#/properties/counts/properties/pending/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
}
}
if(data1.skipped !== undefined){
let data6 = data1.skipped;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
const err24 = {instancePath:instancePath+"/counts/skipped",schemaPath:"#/properties/counts/properties/skipped/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
const err25 = {instancePath:instancePath+"/counts/skipped",schemaPath:"#/properties/counts/properties/skipped/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
}
}
else {
const err26 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
if(data.items !== undefined){
let data7 = data.items;
if(Array.isArray(data7)){
const len0 = data7.length;
for(let i0=0; i0<len0; i0++){
let data8 = data7[i0];
if(data8 && typeof data8 == "object" && !Array.isArray(data8)){
if(data8.operationId === undefined){
const err27 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "operationId"},message:"must have required property '"+"operationId"+"'"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
if(data8.itemId === undefined){
const err28 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
if(data8.source === undefined){
const err29 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "source"},message:"must have required property '"+"source"+"'"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
if(data8.target === undefined){
const err30 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
if(data8.status === undefined){
const err31 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
if(data8.resolution === undefined){
const err32 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "resolution"},message:"must have required property '"+"resolution"+"'"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
if(data8.message === undefined){
const err33 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
for(const key2 in data8){
if(!(((((((key2 === "itemId") || (key2 === "message")) || (key2 === "operationId")) || (key2 === "resolution")) || (key2 === "source")) || (key2 === "status")) || (key2 === "target"))){
const err34 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
}
if(data8.itemId !== undefined){
if(typeof data8.itemId !== "string"){
const err35 = {instancePath:instancePath+"/items/" + i0+"/itemId",schemaPath:"#/properties/items/items/properties/itemId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
}
if(data8.message !== undefined){
if(typeof data8.message !== "string"){
const err36 = {instancePath:instancePath+"/items/" + i0+"/message",schemaPath:"#/properties/items/items/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
}
if(data8.operationId !== undefined){
if(typeof data8.operationId !== "string"){
const err37 = {instancePath:instancePath+"/items/" + i0+"/operationId",schemaPath:"#/properties/items/items/properties/operationId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
}
if(data8.resolution !== undefined){
let data12 = data8.resolution;
const _errs29 = errors;
let valid5 = false;
let passing0 = null;
const _errs30 = errors;
if(typeof data12 !== "string"){
const err38 = {instancePath:instancePath+"/items/" + i0+"/resolution",schemaPath:"#/properties/items/items/properties/resolution/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
if("open" !== data12){
const err39 = {instancePath:instancePath+"/items/" + i0+"/resolution",schemaPath:"#/properties/items/items/properties/resolution/oneOf/0/const",keyword:"const",params:{allowedValue: "open"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
var _valid0 = _errs30 === errors;
if(_valid0){
valid5 = true;
passing0 = 0;
}
const _errs32 = errors;
if(typeof data12 !== "string"){
const err40 = {instancePath:instancePath+"/items/" + i0+"/resolution",schemaPath:"#/properties/items/items/properties/resolution/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
if("acknowledged" !== data12){
const err41 = {instancePath:instancePath+"/items/" + i0+"/resolution",schemaPath:"#/properties/items/items/properties/resolution/oneOf/1/const",keyword:"const",params:{allowedValue: "acknowledged"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
var _valid0 = _errs32 === errors;
if(_valid0 && valid5){
valid5 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid5 = true;
passing0 = 1;
}
}
if(!valid5){
const err42 = {instancePath:instancePath+"/items/" + i0+"/resolution",schemaPath:"#/properties/items/items/properties/resolution/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
else {
errors = _errs29;
if(vErrors !== null){
if(_errs29){
vErrors.length = _errs29;
}
else {
vErrors = null;
}
}
}
}
if(data8.source !== undefined){
let data13 = data8.source;
if(Array.isArray(data13)){
const len1 = data13.length;
for(let i1=0; i1<len1; i1++){
if(typeof data13[i1] !== "string"){
const err43 = {instancePath:instancePath+"/items/" + i0+"/source/" + i1,schemaPath:"#/properties/items/items/properties/source/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
}
}
else {
const err44 = {instancePath:instancePath+"/items/" + i0+"/source",schemaPath:"#/properties/items/items/properties/source/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
}
if(data8.status !== undefined){
let data15 = data8.status;
const _errs39 = errors;
let valid8 = false;
let passing1 = null;
const _errs40 = errors;
if(typeof data15 !== "string"){
const err45 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err45];
}
else {
vErrors.push(err45);
}
errors++;
}
if(!(((data15 === "pending") || (data15 === "applied")) || (data15 === "failed"))){
const err46 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema28.properties.items.items.properties.status.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err46];
}
else {
vErrors.push(err46);
}
errors++;
}
var _valid1 = _errs40 === errors;
if(_valid1){
valid8 = true;
passing1 = 0;
}
const _errs42 = errors;
if(typeof data15 !== "string"){
const err47 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err47];
}
else {
vErrors.push(err47);
}
errors++;
}
if("prepared" !== data15){
const err48 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/1/const",keyword:"const",params:{allowedValue: "prepared"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err48];
}
else {
vErrors.push(err48);
}
errors++;
}
var _valid1 = _errs42 === errors;
if(_valid1 && valid8){
valid8 = false;
passing1 = [passing1, 1];
}
else {
if(_valid1){
valid8 = true;
passing1 = 1;
}
const _errs44 = errors;
if(typeof data15 !== "string"){
const err49 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err49];
}
else {
vErrors.push(err49);
}
errors++;
}
if("skipped" !== data15){
const err50 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/2/const",keyword:"const",params:{allowedValue: "skipped"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err50];
}
else {
vErrors.push(err50);
}
errors++;
}
var _valid1 = _errs44 === errors;
if(_valid1 && valid8){
valid8 = false;
passing1 = [passing1, 2];
}
else {
if(_valid1){
valid8 = true;
passing1 = 2;
}
const _errs46 = errors;
if(typeof data15 !== "string"){
const err51 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err51];
}
else {
vErrors.push(err51);
}
errors++;
}
if("ambiguous" !== data15){
const err52 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf/3/const",keyword:"const",params:{allowedValue: "ambiguous"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err52];
}
else {
vErrors.push(err52);
}
errors++;
}
var _valid1 = _errs46 === errors;
if(_valid1 && valid8){
valid8 = false;
passing1 = [passing1, 3];
}
else {
if(_valid1){
valid8 = true;
passing1 = 3;
}
}
}
}
if(!valid8){
const err53 = {instancePath:instancePath+"/items/" + i0+"/status",schemaPath:"#/properties/items/items/properties/status/oneOf",keyword:"oneOf",params:{passingSchemas: passing1},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err53];
}
else {
vErrors.push(err53);
}
errors++;
}
else {
errors = _errs39;
if(vErrors !== null){
if(_errs39){
vErrors.length = _errs39;
}
else {
vErrors = null;
}
}
}
}
if(data8.target !== undefined){
let data16 = data8.target;
if(Array.isArray(data16)){
const len2 = data16.length;
for(let i2=0; i2<len2; i2++){
if(typeof data16[i2] !== "string"){
const err54 = {instancePath:instancePath+"/items/" + i0+"/target/" + i2,schemaPath:"#/properties/items/items/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err54];
}
else {
vErrors.push(err54);
}
errors++;
}
}
}
else {
const err55 = {instancePath:instancePath+"/items/" + i0+"/target",schemaPath:"#/properties/items/items/properties/target/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err55];
}
else {
vErrors.push(err55);
}
errors++;
}
}
}
else {
const err56 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err56];
}
else {
vErrors.push(err56);
}
errors++;
}
}
}
else {
const err57 = {instancePath:instancePath+"/items",schemaPath:"#/properties/items/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err57];
}
else {
vErrors.push(err57);
}
errors++;
}
}
if(data.planId !== undefined){
if(typeof data.planId !== "string"){
const err58 = {instancePath:instancePath+"/planId",schemaPath:"#/properties/planId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err58];
}
else {
vErrors.push(err58);
}
errors++;
}
}
if(data.rootAuthorized !== undefined){
if(typeof data.rootAuthorized !== "boolean"){
const err59 = {instancePath:instancePath+"/rootAuthorized",schemaPath:"#/properties/rootAuthorized/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err59];
}
else {
vErrors.push(err59);
}
errors++;
}
}
if(data.runId !== undefined){
if(typeof data.runId !== "string"){
const err60 = {instancePath:instancePath+"/runId",schemaPath:"#/properties/runId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err60];
}
else {
vErrors.push(err60);
}
errors++;
}
}
if(data.stateDigest !== undefined){
if(typeof data.stateDigest !== "string"){
const err61 = {instancePath:instancePath+"/stateDigest",schemaPath:"#/properties/stateDigest/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err61];
}
else {
vErrors.push(err61);
}
errors++;
}
}
if(data.status !== undefined){
let data22 = data.status;
if(typeof data22 !== "string"){
const err62 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err62];
}
else {
vErrors.push(err62);
}
errors++;
}
if(!(((((((data22 === "queued") || (data22 === "running")) || (data22 === "completed")) || (data22 === "partial")) || (data22 === "failed")) || (data22 === "cancelled")) || (data22 === "recoveryRequired"))){
const err63 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/enum",keyword:"enum",params:{allowedValues: schema28.properties.status.enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err63];
}
else {
vErrors.push(err63);
}
errors++;
}
}
}
else {
const err64 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err64];
}
else {
vErrors.push(err64);
}
errors++;
}
validate27.errors = vErrors;
return errors === 0;
}

export const RecoveryStatus = validate28;
const schema29 = {"additionalProperties":false,"description":"首页要知道的恢复概况。","properties":{"blocked":{"description":"还有任何未处置的未决项时为 true —— 此时不能开始新任务（规格 8.2）。","type":"boolean"},"blockedRuns":{"items":{"additionalProperties":false,"description":"一个还没处理干净的 run。","properties":{"planId":{"type":"string"},"runId":{"type":"string"},"unresolved":{"description":"还有几项没有被核对处置。","format":"uint32","minimum":0,"type":"integer"}},"required":["runId","planId","unresolved"],"type":"object"},"type":"array"}},"required":["blocked","blockedRuns"],"type":"object"};

function validate28(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.blocked === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "blocked"},message:"must have required property '"+"blocked"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.blockedRuns === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "blockedRuns"},message:"must have required property '"+"blockedRuns"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
for(const key0 in data){
if(!((key0 === "blocked") || (key0 === "blockedRuns"))){
const err2 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
if(data.blocked !== undefined){
if(typeof data.blocked !== "boolean"){
const err3 = {instancePath:instancePath+"/blocked",schemaPath:"#/properties/blocked/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.blockedRuns !== undefined){
let data1 = data.blockedRuns;
if(Array.isArray(data1)){
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
let data2 = data1[i0];
if(data2 && typeof data2 == "object" && !Array.isArray(data2)){
if(data2.runId === undefined){
const err4 = {instancePath:instancePath+"/blockedRuns/" + i0,schemaPath:"#/properties/blockedRuns/items/required",keyword:"required",params:{missingProperty: "runId"},message:"must have required property '"+"runId"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data2.planId === undefined){
const err5 = {instancePath:instancePath+"/blockedRuns/" + i0,schemaPath:"#/properties/blockedRuns/items/required",keyword:"required",params:{missingProperty: "planId"},message:"must have required property '"+"planId"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data2.unresolved === undefined){
const err6 = {instancePath:instancePath+"/blockedRuns/" + i0,schemaPath:"#/properties/blockedRuns/items/required",keyword:"required",params:{missingProperty: "unresolved"},message:"must have required property '"+"unresolved"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key1 in data2){
if(!(((key1 === "planId") || (key1 === "runId")) || (key1 === "unresolved"))){
const err7 = {instancePath:instancePath+"/blockedRuns/" + i0,schemaPath:"#/properties/blockedRuns/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data2.planId !== undefined){
if(typeof data2.planId !== "string"){
const err8 = {instancePath:instancePath+"/blockedRuns/" + i0+"/planId",schemaPath:"#/properties/blockedRuns/items/properties/planId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data2.runId !== undefined){
if(typeof data2.runId !== "string"){
const err9 = {instancePath:instancePath+"/blockedRuns/" + i0+"/runId",schemaPath:"#/properties/blockedRuns/items/properties/runId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data2.unresolved !== undefined){
let data5 = data2.unresolved;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
const err10 = {instancePath:instancePath+"/blockedRuns/" + i0+"/unresolved",schemaPath:"#/properties/blockedRuns/items/properties/unresolved/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
const err11 = {instancePath:instancePath+"/blockedRuns/" + i0+"/unresolved",schemaPath:"#/properties/blockedRuns/items/properties/unresolved/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
}
}
else {
const err12 = {instancePath:instancePath+"/blockedRuns/" + i0,schemaPath:"#/properties/blockedRuns/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
}
}
else {
const err13 = {instancePath:instancePath+"/blockedRuns",schemaPath:"#/properties/blockedRuns/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
}
else {
const err14 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
validate28.errors = vErrors;
return errors === 0;
}

export const RootSummary = validate29;
const schema30 = {"additionalProperties":false,"description":"已授权根目录的摘要。\n\n规格 3.3：前端只拿到 `rootId`，**后续提交 rootId 而不是路径**。\n`displayPath` 仅用于展示，前端不能拿它去拼路径再传回来，\n后端也不会接受前端传来的任何路径字符串。","properties":{"displayPath":{"description":"面向用户的原始路径（可能是长路径或含中文）。","type":"string"},"rootId":{"type":"string"},"volumeId":{"description":"卷标识。前端可利用它提示「根目录换了卷」。","type":"string"}},"required":["rootId","displayPath","volumeId"],"type":"object"};

function validate29(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.rootId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "rootId"},message:"must have required property '"+"rootId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.displayPath === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "displayPath"},message:"must have required property '"+"displayPath"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.volumeId === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "volumeId"},message:"must have required property '"+"volumeId"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
for(const key0 in data){
if(!(((key0 === "displayPath") || (key0 === "rootId")) || (key0 === "volumeId"))){
const err3 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.displayPath !== undefined){
if(typeof data.displayPath !== "string"){
const err4 = {instancePath:instancePath+"/displayPath",schemaPath:"#/properties/displayPath/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
if(data.rootId !== undefined){
if(typeof data.rootId !== "string"){
const err5 = {instancePath:instancePath+"/rootId",schemaPath:"#/properties/rootId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
if(data.volumeId !== undefined){
if(typeof data.volumeId !== "string"){
const err6 = {instancePath:instancePath+"/volumeId",schemaPath:"#/properties/volumeId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
}
else {
const err7 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
validate29.errors = vErrors;
return errors === 0;
}

export const RuleKind = validate30;
const schema31 = {"description":"规则种类。规格 5.2：`create_plan` 的 `ruleKind` 取值 `byType` / `byMonth`。\n\n这里只做 `serde` 而不生成 TypeScript/JSON Schema：T04 还没有接通\n`create_plan` 命令，为一个当前无人消费的类型生成契约定义\n只会让 `contracts:check` 的比对结果与实际用法脱节。\nT05 起它成为**契约类型**：前端要在界面上让用户选按什么规则整理，\n因此和 、 一样由 Rust 生成 TS 类型与 JSON Schema，\n而不是在前端另写一份（那会变成第二套真源）。","oneOf":[{"const":"byType","description":"按扩展名分类：文档 / 图片 / 音视频 / 压缩包 / 其他。","type":"string"},{"const":"byMonth","description":"按**修改**月份分类：`YYYY-MM`。","type":"string"}]};

function validate30(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("byType" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "byType"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("byMonth" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "byMonth"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate30.errors = vErrors;
return errors === 0;
}

export const RunReport = validate31;
const schema32 = {"additionalProperties":false,"description":"执行报告。","properties":{"counts":{"additionalProperties":false,"description":"执行结果计数。","properties":{"ambiguous":{"description":"判定不出来、需要人工核对的项数（规格 8.3）。\n\n**单独计数而不是并进 `pending`**：界面上「还没轮到他」和\n「做了但说不清」需要不同的措辞和不同的动作，混在一起用户无从下手。","format":"uint32","minimum":0,"type":"integer"},"applied":{"format":"uint32","minimum":0,"type":"integer"},"failed":{"format":"uint32","minimum":0,"type":"integer"},"pending":{"format":"uint32","minimum":0,"type":"integer"},"skipped":{"format":"uint32","minimum":0,"type":"integer"}},"required":["applied","failed","skipped","pending","ambiguous"],"type":"object"},"direction":{"description":"`apply`（整理）或 `undo`（撤销）。\n\n历史列表**必须**能分清这两种记录：它们都会出现在同一条时间线上，\n而「已完成 3 项」在整理里是「搬走了 3 个文件」、在撤销里是\n「搬回了 3 个文件」——意思正好相反。少了这个字段，界面只能\n给撤销记录也挂一个「撤销」按钮，而那是一次注定被拒绝的点击。","type":"string"},"issues":{"items":{"additionalProperties":false,"description":"校验或执行过程中的一个问题。","properties":{"code":{"type":"string"},"itemId":{"description":"与具体计划项相关时给出；全局问题为 `None`。","type":["string","null"]},"message":{"type":"string"},"severity":{"description":"校验问题的严重度。","oneOf":[{"enum":["info","warning"],"type":"string"},{"const":"block","description":"阻断项：只要存在，就不能生成可执行计划。","type":"string"}]}},"required":["code","severity","itemId","message"],"type":"object"},"type":"array"},"planId":{"type":"string"},"runId":{"type":"string"},"stateDigest":{"description":"当前操作事实与冲突集合的摘要，用于防止用户确认一份过期的报告。","type":"string"},"status":{"description":"一次执行（含撤销执行）的状态。","enum":["queued","running","completed","partial","failed","cancelled","recoveryRequired"],"type":"string"}},"required":["runId","planId","direction","stateDigest","status","counts","issues"],"type":"object"};

function validate31(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.runId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "runId"},message:"must have required property '"+"runId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.planId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "planId"},message:"must have required property '"+"planId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.direction === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "direction"},message:"must have required property '"+"direction"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.stateDigest === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "stateDigest"},message:"must have required property '"+"stateDigest"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.status === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.counts === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "counts"},message:"must have required property '"+"counts"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.issues === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "issues"},message:"must have required property '"+"issues"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key0 in data){
if(!(((((((key0 === "counts") || (key0 === "direction")) || (key0 === "issues")) || (key0 === "planId")) || (key0 === "runId")) || (key0 === "stateDigest")) || (key0 === "status"))){
const err7 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.counts !== undefined){
let data0 = data.counts;
if(data0 && typeof data0 == "object" && !Array.isArray(data0)){
if(data0.applied === undefined){
const err8 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "applied"},message:"must have required property '"+"applied"+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if(data0.failed === undefined){
const err9 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "failed"},message:"must have required property '"+"failed"+"'"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
if(data0.skipped === undefined){
const err10 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "skipped"},message:"must have required property '"+"skipped"+"'"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if(data0.pending === undefined){
const err11 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "pending"},message:"must have required property '"+"pending"+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if(data0.ambiguous === undefined){
const err12 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/required",keyword:"required",params:{missingProperty: "ambiguous"},message:"must have required property '"+"ambiguous"+"'"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
for(const key1 in data0){
if(!(((((key1 === "ambiguous") || (key1 === "applied")) || (key1 === "failed")) || (key1 === "pending")) || (key1 === "skipped"))){
const err13 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
if(data0.ambiguous !== undefined){
let data1 = data0.ambiguous;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err14 = {instancePath:instancePath+"/counts/ambiguous",schemaPath:"#/properties/counts/properties/ambiguous/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err15 = {instancePath:instancePath+"/counts/ambiguous",schemaPath:"#/properties/counts/properties/ambiguous/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
}
}
if(data0.applied !== undefined){
let data2 = data0.applied;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
const err16 = {instancePath:instancePath+"/counts/applied",schemaPath:"#/properties/counts/properties/applied/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
const err17 = {instancePath:instancePath+"/counts/applied",schemaPath:"#/properties/counts/properties/applied/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
}
}
if(data0.failed !== undefined){
let data3 = data0.failed;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
const err18 = {instancePath:instancePath+"/counts/failed",schemaPath:"#/properties/counts/properties/failed/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
const err19 = {instancePath:instancePath+"/counts/failed",schemaPath:"#/properties/counts/properties/failed/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
}
}
if(data0.pending !== undefined){
let data4 = data0.pending;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
const err20 = {instancePath:instancePath+"/counts/pending",schemaPath:"#/properties/counts/properties/pending/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
const err21 = {instancePath:instancePath+"/counts/pending",schemaPath:"#/properties/counts/properties/pending/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
}
if(data0.skipped !== undefined){
let data5 = data0.skipped;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
const err22 = {instancePath:instancePath+"/counts/skipped",schemaPath:"#/properties/counts/properties/skipped/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
const err23 = {instancePath:instancePath+"/counts/skipped",schemaPath:"#/properties/counts/properties/skipped/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
}
}
}
else {
const err24 = {instancePath:instancePath+"/counts",schemaPath:"#/properties/counts/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
}
if(data.direction !== undefined){
if(typeof data.direction !== "string"){
const err25 = {instancePath:instancePath+"/direction",schemaPath:"#/properties/direction/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
if(data.issues !== undefined){
let data7 = data.issues;
if(Array.isArray(data7)){
const len0 = data7.length;
for(let i0=0; i0<len0; i0++){
let data8 = data7[i0];
if(data8 && typeof data8 == "object" && !Array.isArray(data8)){
if(data8.code === undefined){
const err26 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "code"},message:"must have required property '"+"code"+"'"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
if(data8.severity === undefined){
const err27 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "severity"},message:"must have required property '"+"severity"+"'"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
if(data8.itemId === undefined){
const err28 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
if(data8.message === undefined){
const err29 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
for(const key2 in data8){
if(!((((key2 === "code") || (key2 === "itemId")) || (key2 === "message")) || (key2 === "severity"))){
const err30 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
}
if(data8.code !== undefined){
if(typeof data8.code !== "string"){
const err31 = {instancePath:instancePath+"/issues/" + i0+"/code",schemaPath:"#/properties/issues/items/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
}
if(data8.itemId !== undefined){
let data10 = data8.itemId;
if((typeof data10 !== "string") && (data10 !== null)){
const err32 = {instancePath:instancePath+"/issues/" + i0+"/itemId",schemaPath:"#/properties/issues/items/properties/itemId/type",keyword:"type",params:{type: schema32.properties.issues.items.properties.itemId.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
}
if(data8.message !== undefined){
if(typeof data8.message !== "string"){
const err33 = {instancePath:instancePath+"/issues/" + i0+"/message",schemaPath:"#/properties/issues/items/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
}
if(data8.severity !== undefined){
let data12 = data8.severity;
const _errs29 = errors;
let valid5 = false;
let passing0 = null;
const _errs30 = errors;
if(typeof data12 !== "string"){
const err34 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
if(!((data12 === "info") || (data12 === "warning"))){
const err35 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema32.properties.issues.items.properties.severity.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
var _valid0 = _errs30 === errors;
if(_valid0){
valid5 = true;
passing0 = 0;
}
const _errs32 = errors;
if(typeof data12 !== "string"){
const err36 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
if("block" !== data12){
const err37 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/1/const",keyword:"const",params:{allowedValue: "block"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
var _valid0 = _errs32 === errors;
if(_valid0 && valid5){
valid5 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid5 = true;
passing0 = 1;
}
}
if(!valid5){
const err38 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
else {
errors = _errs29;
if(vErrors !== null){
if(_errs29){
vErrors.length = _errs29;
}
else {
vErrors = null;
}
}
}
}
}
else {
const err39 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
}
}
else {
const err40 = {instancePath:instancePath+"/issues",schemaPath:"#/properties/issues/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
}
if(data.planId !== undefined){
if(typeof data.planId !== "string"){
const err41 = {instancePath:instancePath+"/planId",schemaPath:"#/properties/planId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
}
if(data.runId !== undefined){
if(typeof data.runId !== "string"){
const err42 = {instancePath:instancePath+"/runId",schemaPath:"#/properties/runId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
}
if(data.stateDigest !== undefined){
if(typeof data.stateDigest !== "string"){
const err43 = {instancePath:instancePath+"/stateDigest",schemaPath:"#/properties/stateDigest/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
}
if(data.status !== undefined){
let data16 = data.status;
if(typeof data16 !== "string"){
const err44 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
if(!(((((((data16 === "queued") || (data16 === "running")) || (data16 === "completed")) || (data16 === "partial")) || (data16 === "failed")) || (data16 === "cancelled")) || (data16 === "recoveryRequired"))){
const err45 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/enum",keyword:"enum",params:{allowedValues: schema32.properties.status.enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err45];
}
else {
vErrors.push(err45);
}
errors++;
}
}
}
else {
const err46 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err46];
}
else {
vErrors.push(err46);
}
errors++;
}
validate31.errors = vErrors;
return errors === 0;
}

export const ScanSummary = validate32;
const schema33 = {"additionalProperties":false,"description":"一次扫描任务的结果摘要。","properties":{"rootId":{"type":"string"},"scanId":{"type":"string"},"skipped":{"description":"被跳过的条目数。","format":"uint32","minimum":0,"type":"integer"},"taskId":{"description":"规格 5.2：`start_scan` 返回 taskId。","type":"string"},"total":{"description":"枚举到的条目总数（含被跳过的）。","format":"uint32","minimum":0,"type":"integer"},"truncated":{"description":"规格 6.1：达到数量/深度上限时为 true，界面必须要求缩小范围。","type":"boolean"},"usable":{"description":"可参与整理的普通文件数。","format":"uint32","minimum":0,"type":"integer"}},"required":["taskId","scanId","rootId","total","usable","skipped","truncated"],"type":"object"};

function validate32(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.taskId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "taskId"},message:"must have required property '"+"taskId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.scanId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "scanId"},message:"must have required property '"+"scanId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.rootId === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "rootId"},message:"must have required property '"+"rootId"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.total === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "total"},message:"must have required property '"+"total"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.usable === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "usable"},message:"must have required property '"+"usable"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.skipped === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "skipped"},message:"must have required property '"+"skipped"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.truncated === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "truncated"},message:"must have required property '"+"truncated"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key0 in data){
if(!(((((((key0 === "rootId") || (key0 === "scanId")) || (key0 === "skipped")) || (key0 === "taskId")) || (key0 === "total")) || (key0 === "truncated")) || (key0 === "usable"))){
const err7 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.rootId !== undefined){
if(typeof data.rootId !== "string"){
const err8 = {instancePath:instancePath+"/rootId",schemaPath:"#/properties/rootId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.scanId !== undefined){
if(typeof data.scanId !== "string"){
const err9 = {instancePath:instancePath+"/scanId",schemaPath:"#/properties/scanId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.skipped !== undefined){
let data2 = data.skipped;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
const err10 = {instancePath:instancePath+"/skipped",schemaPath:"#/properties/skipped/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
const err11 = {instancePath:instancePath+"/skipped",schemaPath:"#/properties/skipped/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
}
if(data.taskId !== undefined){
if(typeof data.taskId !== "string"){
const err12 = {instancePath:instancePath+"/taskId",schemaPath:"#/properties/taskId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
}
if(data.total !== undefined){
let data4 = data.total;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
const err13 = {instancePath:instancePath+"/total",schemaPath:"#/properties/total/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
const err14 = {instancePath:instancePath+"/total",schemaPath:"#/properties/total/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
}
}
if(data.truncated !== undefined){
if(typeof data.truncated !== "boolean"){
const err15 = {instancePath:instancePath+"/truncated",schemaPath:"#/properties/truncated/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
}
if(data.usable !== undefined){
let data6 = data.usable;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
const err16 = {instancePath:instancePath+"/usable",schemaPath:"#/properties/usable/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
const err17 = {instancePath:instancePath+"/usable",schemaPath:"#/properties/usable/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
}
}
}
else {
const err18 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
validate32.errors = vErrors;
return errors === 0;
}

export const TaskError = validate33;
const schema34 = {"additionalProperties":false,"description":"任务失败原因的最小结构。\n\n与 `AppError` 字段一致，但定义在 `types` 里以避免 `types` ↔ `errors` 互相依赖。","properties":{"code":{"type":"string"},"message":{"type":"string"},"retryable":{"type":"boolean"}},"required":["code","message","retryable"],"type":"object"};

function validate33(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.code === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "code"},message:"must have required property '"+"code"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.message === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.retryable === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "retryable"},message:"must have required property '"+"retryable"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
for(const key0 in data){
if(!(((key0 === "code") || (key0 === "message")) || (key0 === "retryable"))){
const err3 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
if(data.code !== undefined){
if(typeof data.code !== "string"){
const err4 = {instancePath:instancePath+"/code",schemaPath:"#/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
if(data.message !== undefined){
if(typeof data.message !== "string"){
const err5 = {instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
if(data.retryable !== undefined){
if(typeof data.retryable !== "boolean"){
const err6 = {instancePath:instancePath+"/retryable",schemaPath:"#/properties/retryable/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
}
else {
const err7 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
validate33.errors = vErrors;
return errors === 0;
}

export const TaskSummary = validate34;
const schema35 = {"additionalProperties":false,"description":"任务状态查询结果。\n\n规格 5.2：页面切换不得终止任务，重新打开界面要能用 `taskId` 查回来。","properties":{"error":{"additionalProperties":false,"description":"失败时的结构化错误；成功或进行中为 `null`。\n\n注意：`AppError` 在 `domain::errors` 里，这里用 `crate::domain::errors::AppError`\n会导致 types.rs 反向依赖 errors.rs。为避免循环，改用脱敏后的最小结构。","properties":{"code":{"type":"string"},"message":{"type":"string"},"retryable":{"type":"boolean"}},"required":["code","message","retryable"],"type":["object","null"]},"processed":{"format":"uint32","minimum":0,"type":"integer"},"scanId":{"description":"扫描类任务完成后的 scanId；其他任务为 `null`。","type":["string","null"]},"status":{"description":"后台任务状态。","oneOf":[{"enum":["queued","running","completed","failed","cancelled"],"type":"string"},{"const":"partial","description":"部分完成：首个失败即停止，已完成项保持完成，不自动回滚。","type":"string"},{"const":"recoveryRequired","description":"存在无法自动判定的执行状态，必须先完成恢复核对。","type":"string"}]},"taskId":{"type":"string"},"total":{"description":"总量尚未确定时为 `null`。","format":"uint32","minimum":0,"type":["integer","null"]}},"required":["taskId","status","processed","total","scanId","error"],"type":"object"};

function validate34(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.taskId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "taskId"},message:"must have required property '"+"taskId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.status === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.processed === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "processed"},message:"must have required property '"+"processed"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.total === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "total"},message:"must have required property '"+"total"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.scanId === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "scanId"},message:"must have required property '"+"scanId"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.error === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "error"},message:"must have required property '"+"error"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
for(const key0 in data){
if(!((((((key0 === "error") || (key0 === "processed")) || (key0 === "scanId")) || (key0 === "status")) || (key0 === "taskId")) || (key0 === "total"))){
const err6 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
if(data.error !== undefined){
let data0 = data.error;
if((!(data0 && typeof data0 == "object" && !Array.isArray(data0))) && (data0 !== null)){
const err7 = {instancePath:instancePath+"/error",schemaPath:"#/properties/error/type",keyword:"type",params:{type: schema35.properties.error.type},message:"must be object,null"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(data0 && typeof data0 == "object" && !Array.isArray(data0)){
if(data0.code === undefined){
const err8 = {instancePath:instancePath+"/error",schemaPath:"#/properties/error/required",keyword:"required",params:{missingProperty: "code"},message:"must have required property '"+"code"+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if(data0.message === undefined){
const err9 = {instancePath:instancePath+"/error",schemaPath:"#/properties/error/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
if(data0.retryable === undefined){
const err10 = {instancePath:instancePath+"/error",schemaPath:"#/properties/error/required",keyword:"required",params:{missingProperty: "retryable"},message:"must have required property '"+"retryable"+"'"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
for(const key1 in data0){
if(!(((key1 === "code") || (key1 === "message")) || (key1 === "retryable"))){
const err11 = {instancePath:instancePath+"/error",schemaPath:"#/properties/error/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
if(data0.code !== undefined){
if(typeof data0.code !== "string"){
const err12 = {instancePath:instancePath+"/error/code",schemaPath:"#/properties/error/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
}
if(data0.message !== undefined){
if(typeof data0.message !== "string"){
const err13 = {instancePath:instancePath+"/error/message",schemaPath:"#/properties/error/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
if(data0.retryable !== undefined){
if(typeof data0.retryable !== "boolean"){
const err14 = {instancePath:instancePath+"/error/retryable",schemaPath:"#/properties/error/properties/retryable/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
}
}
}
if(data.processed !== undefined){
let data4 = data.processed;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
const err15 = {instancePath:instancePath+"/processed",schemaPath:"#/properties/processed/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
const err16 = {instancePath:instancePath+"/processed",schemaPath:"#/properties/processed/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
}
if(data.scanId !== undefined){
let data5 = data.scanId;
if((typeof data5 !== "string") && (data5 !== null)){
const err17 = {instancePath:instancePath+"/scanId",schemaPath:"#/properties/scanId/type",keyword:"type",params:{type: schema35.properties.scanId.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
}
if(data.status !== undefined){
let data6 = data.status;
const _errs16 = errors;
let valid2 = false;
let passing0 = null;
const _errs17 = errors;
if(typeof data6 !== "string"){
const err18 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if(!(((((data6 === "queued") || (data6 === "running")) || (data6 === "completed")) || (data6 === "failed")) || (data6 === "cancelled"))){
const err19 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema35.properties.status.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
var _valid0 = _errs17 === errors;
if(_valid0){
valid2 = true;
passing0 = 0;
}
const _errs19 = errors;
if(typeof data6 !== "string"){
const err20 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if("partial" !== data6){
const err21 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/1/const",keyword:"const",params:{allowedValue: "partial"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
var _valid0 = _errs19 === errors;
if(_valid0 && valid2){
valid2 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid2 = true;
passing0 = 1;
}
const _errs21 = errors;
if(typeof data6 !== "string"){
const err22 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if("recoveryRequired" !== data6){
const err23 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf/2/const",keyword:"const",params:{allowedValue: "recoveryRequired"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
var _valid0 = _errs21 === errors;
if(_valid0 && valid2){
valid2 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid2 = true;
passing0 = 2;
}
}
}
if(!valid2){
const err24 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
else {
errors = _errs16;
if(vErrors !== null){
if(_errs16){
vErrors.length = _errs16;
}
else {
vErrors = null;
}
}
}
}
if(data.taskId !== undefined){
if(typeof data.taskId !== "string"){
const err25 = {instancePath:instancePath+"/taskId",schemaPath:"#/properties/taskId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
if(data.total !== undefined){
let data8 = data.total;
if((!(((typeof data8 == "number") && (!(data8 % 1) && !isNaN(data8))) && (isFinite(data8)))) && (data8 !== null)){
const err26 = {instancePath:instancePath+"/total",schemaPath:"#/properties/total/type",keyword:"type",params:{type: schema35.properties.total.type},message:"must be integer,null"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
if((typeof data8 == "number") && (isFinite(data8))){
if(data8 < 0 || isNaN(data8)){
const err27 = {instancePath:instancePath+"/total",schemaPath:"#/properties/total/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
}
}
}
else {
const err28 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
validate34.errors = vErrors;
return errors === 0;
}

export const UndoItem = validate35;
const schema36 = {"additionalProperties":false,"description":"撤销预览中的一项。","properties":{"itemId":{"type":"string"},"message":{"description":"判定原因，直接显示在界面上。","type":"string"},"operationId":{"description":"被撤销的那个原操作。","type":"string"},"outcome":{"description":"撤销预览中单项的判定。\n\n与 `OpStatus` 是**不同维度**：`OpStatus` 说的是「原操作当初做成了没有」，\n这里说的是「**现在**能不能把它移回去」。同一项完全可以是\n`status = applied` 而 `outcome = conflict`——那正是「整理后被改动」的情形。","oneOf":[{"const":"ready","description":"条件都满足，可以安全移回。**默认选中**。","type":"string"},{"const":"conflict","description":"原路径被占用、内容被改动，或原父目录消失。**默认不选中**。","type":"string"},{"const":"alreadyUndone","description":"之前已经撤销过。重复撤销只回报「已完成」，绝不再搬动文件。","type":"string"}]},"selected":{"description":"是否被选中执行。`Conflict` 与 `AlreadyUndone` 初始为 false。","type":"boolean"},"source":{"description":"撤销时的**源** = 原操作的目标位置。","items":{"type":"string"},"type":"array"},"target":{"description":"撤销时的**目标** = 原操作的源位置。","items":{"type":"string"},"type":"array"}},"required":["operationId","itemId","source","target","outcome","selected","message"],"type":"object"};

function validate35(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.operationId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "operationId"},message:"must have required property '"+"operationId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.itemId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.source === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "source"},message:"must have required property '"+"source"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.target === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.outcome === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "outcome"},message:"must have required property '"+"outcome"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.selected === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "selected"},message:"must have required property '"+"selected"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.message === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key0 in data){
if(!(((((((key0 === "itemId") || (key0 === "message")) || (key0 === "operationId")) || (key0 === "outcome")) || (key0 === "selected")) || (key0 === "source")) || (key0 === "target"))){
const err7 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.itemId !== undefined){
if(typeof data.itemId !== "string"){
const err8 = {instancePath:instancePath+"/itemId",schemaPath:"#/properties/itemId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.message !== undefined){
if(typeof data.message !== "string"){
const err9 = {instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.operationId !== undefined){
if(typeof data.operationId !== "string"){
const err10 = {instancePath:instancePath+"/operationId",schemaPath:"#/properties/operationId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
if(data.outcome !== undefined){
let data3 = data.outcome;
const _errs9 = errors;
let valid1 = false;
let passing0 = null;
const _errs10 = errors;
if(typeof data3 !== "string"){
const err11 = {instancePath:instancePath+"/outcome",schemaPath:"#/properties/outcome/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if("ready" !== data3){
const err12 = {instancePath:instancePath+"/outcome",schemaPath:"#/properties/outcome/oneOf/0/const",keyword:"const",params:{allowedValue: "ready"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
var _valid0 = _errs10 === errors;
if(_valid0){
valid1 = true;
passing0 = 0;
}
const _errs12 = errors;
if(typeof data3 !== "string"){
const err13 = {instancePath:instancePath+"/outcome",schemaPath:"#/properties/outcome/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if("conflict" !== data3){
const err14 = {instancePath:instancePath+"/outcome",schemaPath:"#/properties/outcome/oneOf/1/const",keyword:"const",params:{allowedValue: "conflict"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
var _valid0 = _errs12 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid1 = true;
passing0 = 1;
}
const _errs14 = errors;
if(typeof data3 !== "string"){
const err15 = {instancePath:instancePath+"/outcome",schemaPath:"#/properties/outcome/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
if("alreadyUndone" !== data3){
const err16 = {instancePath:instancePath+"/outcome",schemaPath:"#/properties/outcome/oneOf/2/const",keyword:"const",params:{allowedValue: "alreadyUndone"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
var _valid0 = _errs14 === errors;
if(_valid0 && valid1){
valid1 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid1 = true;
passing0 = 2;
}
}
}
if(!valid1){
const err17 = {instancePath:instancePath+"/outcome",schemaPath:"#/properties/outcome/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
else {
errors = _errs9;
if(vErrors !== null){
if(_errs9){
vErrors.length = _errs9;
}
else {
vErrors = null;
}
}
}
}
if(data.selected !== undefined){
if(typeof data.selected !== "boolean"){
const err18 = {instancePath:instancePath+"/selected",schemaPath:"#/properties/selected/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
}
if(data.source !== undefined){
let data5 = data.source;
if(Array.isArray(data5)){
const len0 = data5.length;
for(let i0=0; i0<len0; i0++){
if(typeof data5[i0] !== "string"){
const err19 = {instancePath:instancePath+"/source/" + i0,schemaPath:"#/properties/source/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
}
}
else {
const err20 = {instancePath:instancePath+"/source",schemaPath:"#/properties/source/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
}
if(data.target !== undefined){
let data7 = data.target;
if(Array.isArray(data7)){
const len1 = data7.length;
for(let i1=0; i1<len1; i1++){
if(typeof data7[i1] !== "string"){
const err21 = {instancePath:instancePath+"/target/" + i1,schemaPath:"#/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
}
else {
const err22 = {instancePath:instancePath+"/target",schemaPath:"#/properties/target/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
}
}
else {
const err23 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
validate35.errors = vErrors;
return errors === 0;
}

export const UndoOutcome = validate36;
const schema37 = {"description":"撤销预览中单项的判定。\n\n与 `OpStatus` 是**不同维度**：`OpStatus` 说的是「原操作当初做成了没有」，\n这里说的是「**现在**能不能把它移回去」。同一项完全可以是\n`status = applied` 而 `outcome = conflict`——那正是「整理后被改动」的情形。","oneOf":[{"const":"ready","description":"条件都满足，可以安全移回。**默认选中**。","type":"string"},{"const":"conflict","description":"原路径被占用、内容被改动，或原父目录消失。**默认不选中**。","type":"string"},{"const":"alreadyUndone","description":"之前已经撤销过。重复撤销只回报「已完成」，绝不再搬动文件。","type":"string"}]};

function validate36(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("ready" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "ready"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("conflict" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "conflict"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
const _errs5 = errors;
if(typeof data !== "string"){
const err4 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if("alreadyUndone" !== data){
const err5 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "alreadyUndone"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
}
}
if(!valid0){
const err6 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate36.errors = vErrors;
return errors === 0;
}

export const UndoPreview = validate37;
const schema38 = {"additionalProperties":false,"description":"撤销预览（规格 8.4）。\n\n与执行前的计划预览同构：**也有摘要和一次性确认**，\n因为撤销同样是一次会改动用户文件的操作，不是异常后的无条件补偿。","properties":{"alreadyUndoneCount":{"description":"已经撤销过的项数（重复撤销时它们不会再次被搬动）。","format":"uint32","minimum":0,"type":"integer"},"conflictCount":{"description":"有冲突的项数。","format":"uint32","minimum":0,"type":"integer"},"digest":{"description":"当前撤销事实的摘要。执行时必须原样带回，变了就说明这一屏已经过期。","type":"string"},"expiresAt":{"description":"UTC RFC3339，最多 5 分钟有效。","type":["string","null"]},"items":{"items":{"additionalProperties":false,"description":"撤销预览中的一项。","properties":{"itemId":{"type":"string"},"message":{"description":"判定原因，直接显示在界面上。","type":"string"},"operationId":{"description":"被撤销的那个原操作。","type":"string"},"outcome":{"description":"撤销预览中单项的判定。\n\n与 `OpStatus` 是**不同维度**：`OpStatus` 说的是「原操作当初做成了没有」，\n这里说的是「**现在**能不能把它移回去」。同一项完全可以是\n`status = applied` 而 `outcome = conflict`——那正是「整理后被改动」的情形。","oneOf":[{"const":"ready","description":"条件都满足，可以安全移回。**默认选中**。","type":"string"},{"const":"conflict","description":"原路径被占用、内容被改动，或原父目录消失。**默认不选中**。","type":"string"},{"const":"alreadyUndone","description":"之前已经撤销过。重复撤销只回报「已完成」，绝不再搬动文件。","type":"string"}]},"selected":{"description":"是否被选中执行。`Conflict` 与 `AlreadyUndone` 初始为 false。","type":"boolean"},"source":{"description":"撤销时的**源** = 原操作的目标位置。","items":{"type":"string"},"type":"array"},"target":{"description":"撤销时的**目标** = 原操作的源位置。","items":{"type":"string"},"type":"array"}},"required":["operationId","itemId","source","target","outcome","selected","message"],"type":"object"},"type":"array"},"originalRunId":{"type":"string"},"readyCount":{"description":"可安全撤销的项数。","format":"uint32","minimum":0,"type":"integer"},"undoPlanId":{"description":"本次预览落库的撤销计划 id。执行时原样带回，用来消费一次性令牌。","type":"string"},"undoToken":{"description":"一次性令牌。**只有**后端能签发；前端不得自行生成。","type":["string","null"]}},"required":["undoPlanId","originalRunId","digest","items","readyCount","conflictCount","alreadyUndoneCount","undoToken","expiresAt"],"type":"object"};

function validate37(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.undoPlanId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "undoPlanId"},message:"must have required property '"+"undoPlanId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.originalRunId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "originalRunId"},message:"must have required property '"+"originalRunId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.digest === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "digest"},message:"must have required property '"+"digest"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.items === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "items"},message:"must have required property '"+"items"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.readyCount === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "readyCount"},message:"must have required property '"+"readyCount"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.conflictCount === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "conflictCount"},message:"must have required property '"+"conflictCount"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.alreadyUndoneCount === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "alreadyUndoneCount"},message:"must have required property '"+"alreadyUndoneCount"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if(data.undoToken === undefined){
const err7 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "undoToken"},message:"must have required property '"+"undoToken"+"'"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(data.expiresAt === undefined){
const err8 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "expiresAt"},message:"must have required property '"+"expiresAt"+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
for(const key0 in data){
if(!(func2.call(schema38.properties, key0))){
const err9 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.alreadyUndoneCount !== undefined){
let data0 = data.alreadyUndoneCount;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
const err10 = {instancePath:instancePath+"/alreadyUndoneCount",schemaPath:"#/properties/alreadyUndoneCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
const err11 = {instancePath:instancePath+"/alreadyUndoneCount",schemaPath:"#/properties/alreadyUndoneCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
}
if(data.conflictCount !== undefined){
let data1 = data.conflictCount;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err12 = {instancePath:instancePath+"/conflictCount",schemaPath:"#/properties/conflictCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err13 = {instancePath:instancePath+"/conflictCount",schemaPath:"#/properties/conflictCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
}
if(data.digest !== undefined){
if(typeof data.digest !== "string"){
const err14 = {instancePath:instancePath+"/digest",schemaPath:"#/properties/digest/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
}
if(data.expiresAt !== undefined){
let data3 = data.expiresAt;
if((typeof data3 !== "string") && (data3 !== null)){
const err15 = {instancePath:instancePath+"/expiresAt",schemaPath:"#/properties/expiresAt/type",keyword:"type",params:{type: schema38.properties.expiresAt.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
}
if(data.items !== undefined){
let data4 = data.items;
if(Array.isArray(data4)){
const len0 = data4.length;
for(let i0=0; i0<len0; i0++){
let data5 = data4[i0];
if(data5 && typeof data5 == "object" && !Array.isArray(data5)){
if(data5.operationId === undefined){
const err16 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "operationId"},message:"must have required property '"+"operationId"+"'"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if(data5.itemId === undefined){
const err17 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
if(data5.source === undefined){
const err18 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "source"},message:"must have required property '"+"source"+"'"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if(data5.target === undefined){
const err19 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
if(data5.outcome === undefined){
const err20 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "outcome"},message:"must have required property '"+"outcome"+"'"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if(data5.selected === undefined){
const err21 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "selected"},message:"must have required property '"+"selected"+"'"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
if(data5.message === undefined){
const err22 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
for(const key1 in data5){
if(!(((((((key1 === "itemId") || (key1 === "message")) || (key1 === "operationId")) || (key1 === "outcome")) || (key1 === "selected")) || (key1 === "source")) || (key1 === "target"))){
const err23 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
}
if(data5.itemId !== undefined){
if(typeof data5.itemId !== "string"){
const err24 = {instancePath:instancePath+"/items/" + i0+"/itemId",schemaPath:"#/properties/items/items/properties/itemId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
}
if(data5.message !== undefined){
if(typeof data5.message !== "string"){
const err25 = {instancePath:instancePath+"/items/" + i0+"/message",schemaPath:"#/properties/items/items/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
if(data5.operationId !== undefined){
if(typeof data5.operationId !== "string"){
const err26 = {instancePath:instancePath+"/items/" + i0+"/operationId",schemaPath:"#/properties/items/items/properties/operationId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
if(data5.outcome !== undefined){
let data9 = data5.outcome;
const _errs22 = errors;
let valid4 = false;
let passing0 = null;
const _errs23 = errors;
if(typeof data9 !== "string"){
const err27 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
if("ready" !== data9){
const err28 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/0/const",keyword:"const",params:{allowedValue: "ready"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
var _valid0 = _errs23 === errors;
if(_valid0){
valid4 = true;
passing0 = 0;
}
const _errs25 = errors;
if(typeof data9 !== "string"){
const err29 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
if("conflict" !== data9){
const err30 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/1/const",keyword:"const",params:{allowedValue: "conflict"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
var _valid0 = _errs25 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid4 = true;
passing0 = 1;
}
const _errs27 = errors;
if(typeof data9 !== "string"){
const err31 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
if("alreadyUndone" !== data9){
const err32 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/2/const",keyword:"const",params:{allowedValue: "alreadyUndone"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
var _valid0 = _errs27 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid4 = true;
passing0 = 2;
}
}
}
if(!valid4){
const err33 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
else {
errors = _errs22;
if(vErrors !== null){
if(_errs22){
vErrors.length = _errs22;
}
else {
vErrors = null;
}
}
}
}
if(data5.selected !== undefined){
if(typeof data5.selected !== "boolean"){
const err34 = {instancePath:instancePath+"/items/" + i0+"/selected",schemaPath:"#/properties/items/items/properties/selected/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
}
if(data5.source !== undefined){
let data11 = data5.source;
if(Array.isArray(data11)){
const len1 = data11.length;
for(let i1=0; i1<len1; i1++){
if(typeof data11[i1] !== "string"){
const err35 = {instancePath:instancePath+"/items/" + i0+"/source/" + i1,schemaPath:"#/properties/items/items/properties/source/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
}
}
else {
const err36 = {instancePath:instancePath+"/items/" + i0+"/source",schemaPath:"#/properties/items/items/properties/source/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
}
if(data5.target !== undefined){
let data13 = data5.target;
if(Array.isArray(data13)){
const len2 = data13.length;
for(let i2=0; i2<len2; i2++){
if(typeof data13[i2] !== "string"){
const err37 = {instancePath:instancePath+"/items/" + i0+"/target/" + i2,schemaPath:"#/properties/items/items/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
}
}
else {
const err38 = {instancePath:instancePath+"/items/" + i0+"/target",schemaPath:"#/properties/items/items/properties/target/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
}
}
else {
const err39 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
}
}
else {
const err40 = {instancePath:instancePath+"/items",schemaPath:"#/properties/items/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
}
if(data.originalRunId !== undefined){
if(typeof data.originalRunId !== "string"){
const err41 = {instancePath:instancePath+"/originalRunId",schemaPath:"#/properties/originalRunId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
}
if(data.readyCount !== undefined){
let data16 = data.readyCount;
if(!(((typeof data16 == "number") && (!(data16 % 1) && !isNaN(data16))) && (isFinite(data16)))){
const err42 = {instancePath:instancePath+"/readyCount",schemaPath:"#/properties/readyCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
if((typeof data16 == "number") && (isFinite(data16))){
if(data16 < 0 || isNaN(data16)){
const err43 = {instancePath:instancePath+"/readyCount",schemaPath:"#/properties/readyCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
}
}
if(data.undoPlanId !== undefined){
if(typeof data.undoPlanId !== "string"){
const err44 = {instancePath:instancePath+"/undoPlanId",schemaPath:"#/properties/undoPlanId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
}
if(data.undoToken !== undefined){
let data18 = data.undoToken;
if((typeof data18 !== "string") && (data18 !== null)){
const err45 = {instancePath:instancePath+"/undoToken",schemaPath:"#/properties/undoToken/type",keyword:"type",params:{type: schema38.properties.undoToken.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err45];
}
else {
vErrors.push(err45);
}
errors++;
}
}
}
else {
const err46 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err46];
}
else {
vErrors.push(err46);
}
errors++;
}
validate37.errors = vErrors;
return errors === 0;
}

export const UndoReport = validate38;
const schema39 = {"additionalProperties":false,"description":"撤销执行的分项结果（规格 T09）。\n\n「已撤销 / 有冲突 / 未处理」分开计数是硬要求：把部分撤销标成全部成功，\n会让用户以为文件都回去了，而实际上还有几项留在原地。","properties":{"alreadyUndone":{"description":"本次之前就已经撤销过、这次只是再次确认的项数。","format":"uint32","minimum":0,"type":"integer"},"conflicted":{"description":"因冲突而保留现状的项数。","format":"uint32","minimum":0,"type":"integer"},"items":{"items":{"additionalProperties":false,"description":"撤销预览中的一项。","properties":{"itemId":{"type":"string"},"message":{"description":"判定原因，直接显示在界面上。","type":"string"},"operationId":{"description":"被撤销的那个原操作。","type":"string"},"outcome":{"description":"撤销预览中单项的判定。\n\n与 `OpStatus` 是**不同维度**：`OpStatus` 说的是「原操作当初做成了没有」，\n这里说的是「**现在**能不能把它移回去」。同一项完全可以是\n`status = applied` 而 `outcome = conflict`——那正是「整理后被改动」的情形。","oneOf":[{"const":"ready","description":"条件都满足，可以安全移回。**默认选中**。","type":"string"},{"const":"conflict","description":"原路径被占用、内容被改动，或原父目录消失。**默认不选中**。","type":"string"},{"const":"alreadyUndone","description":"之前已经撤销过。重复撤销只回报「已完成」，绝不再搬动文件。","type":"string"}]},"selected":{"description":"是否被选中执行。`Conflict` 与 `AlreadyUndone` 初始为 false。","type":"boolean"},"source":{"description":"撤销时的**源** = 原操作的目标位置。","items":{"type":"string"},"type":"array"},"target":{"description":"撤销时的**目标** = 原操作的源位置。","items":{"type":"string"},"type":"array"}},"required":["operationId","itemId","source","target","outcome","selected","message"],"type":"object"},"type":"array"},"originalRunId":{"type":"string"},"reverted":{"description":"已成功移回的项数。","format":"uint32","minimum":0,"type":"integer"},"runId":{"description":"本次撤销产生的 run（`direction = 'undo'`，可用 `get_run` 查询）。","type":"string"},"status":{"description":"一次执行（含撤销执行）的状态。","enum":["queued","running","completed","partial","failed","cancelled","recoveryRequired"],"type":"string"},"untouched":{"description":"用户没选中、因而未处理的项数。","format":"uint32","minimum":0,"type":"integer"},"warnings":{"description":"目录清理失败等**不影响已移回文件事实**的告警。\n\n单独一个列表而不是塞进 `items`：文件已经安全回去了，\n把「空目录没删掉」混进分项结果会让用户以为撤销失败了。","items":{"additionalProperties":false,"description":"校验或执行过程中的一个问题。","properties":{"code":{"type":"string"},"itemId":{"description":"与具体计划项相关时给出；全局问题为 `None`。","type":["string","null"]},"message":{"type":"string"},"severity":{"description":"校验问题的严重度。","oneOf":[{"enum":["info","warning"],"type":"string"},{"const":"block","description":"阻断项：只要存在，就不能生成可执行计划。","type":"string"}]}},"required":["code","severity","itemId","message"],"type":"object"},"type":"array"}},"required":["runId","originalRunId","status","reverted","conflicted","alreadyUndone","untouched","items","warnings"],"type":"object"};

function validate38(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.runId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "runId"},message:"must have required property '"+"runId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.originalRunId === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "originalRunId"},message:"must have required property '"+"originalRunId"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.status === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "status"},message:"must have required property '"+"status"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.reverted === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "reverted"},message:"must have required property '"+"reverted"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.conflicted === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "conflicted"},message:"must have required property '"+"conflicted"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.alreadyUndone === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "alreadyUndone"},message:"must have required property '"+"alreadyUndone"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.untouched === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "untouched"},message:"must have required property '"+"untouched"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if(data.items === undefined){
const err7 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "items"},message:"must have required property '"+"items"+"'"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(data.warnings === undefined){
const err8 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "warnings"},message:"must have required property '"+"warnings"+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
for(const key0 in data){
if(!(func2.call(schema39.properties, key0))){
const err9 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
if(data.alreadyUndone !== undefined){
let data0 = data.alreadyUndone;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
const err10 = {instancePath:instancePath+"/alreadyUndone",schemaPath:"#/properties/alreadyUndone/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
const err11 = {instancePath:instancePath+"/alreadyUndone",schemaPath:"#/properties/alreadyUndone/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
}
if(data.conflicted !== undefined){
let data1 = data.conflicted;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err12 = {instancePath:instancePath+"/conflicted",schemaPath:"#/properties/conflicted/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err13 = {instancePath:instancePath+"/conflicted",schemaPath:"#/properties/conflicted/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
}
if(data.items !== undefined){
let data2 = data.items;
if(Array.isArray(data2)){
const len0 = data2.length;
for(let i0=0; i0<len0; i0++){
let data3 = data2[i0];
if(data3 && typeof data3 == "object" && !Array.isArray(data3)){
if(data3.operationId === undefined){
const err14 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "operationId"},message:"must have required property '"+"operationId"+"'"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if(data3.itemId === undefined){
const err15 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
if(data3.source === undefined){
const err16 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "source"},message:"must have required property '"+"source"+"'"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
if(data3.target === undefined){
const err17 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "target"},message:"must have required property '"+"target"+"'"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
if(data3.outcome === undefined){
const err18 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "outcome"},message:"must have required property '"+"outcome"+"'"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
if(data3.selected === undefined){
const err19 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "selected"},message:"must have required property '"+"selected"+"'"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
if(data3.message === undefined){
const err20 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
for(const key1 in data3){
if(!(((((((key1 === "itemId") || (key1 === "message")) || (key1 === "operationId")) || (key1 === "outcome")) || (key1 === "selected")) || (key1 === "source")) || (key1 === "target"))){
const err21 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
if(data3.itemId !== undefined){
if(typeof data3.itemId !== "string"){
const err22 = {instancePath:instancePath+"/items/" + i0+"/itemId",schemaPath:"#/properties/items/items/properties/itemId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
}
if(data3.message !== undefined){
if(typeof data3.message !== "string"){
const err23 = {instancePath:instancePath+"/items/" + i0+"/message",schemaPath:"#/properties/items/items/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
}
if(data3.operationId !== undefined){
if(typeof data3.operationId !== "string"){
const err24 = {instancePath:instancePath+"/items/" + i0+"/operationId",schemaPath:"#/properties/items/items/properties/operationId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
}
if(data3.outcome !== undefined){
let data7 = data3.outcome;
const _errs18 = errors;
let valid4 = false;
let passing0 = null;
const _errs19 = errors;
if(typeof data7 !== "string"){
const err25 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
if("ready" !== data7){
const err26 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/0/const",keyword:"const",params:{allowedValue: "ready"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
var _valid0 = _errs19 === errors;
if(_valid0){
valid4 = true;
passing0 = 0;
}
const _errs21 = errors;
if(typeof data7 !== "string"){
const err27 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
if("conflict" !== data7){
const err28 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/1/const",keyword:"const",params:{allowedValue: "conflict"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
var _valid0 = _errs21 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid4 = true;
passing0 = 1;
}
const _errs23 = errors;
if(typeof data7 !== "string"){
const err29 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
if("alreadyUndone" !== data7){
const err30 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf/2/const",keyword:"const",params:{allowedValue: "alreadyUndone"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
var _valid0 = _errs23 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid4 = true;
passing0 = 2;
}
}
}
if(!valid4){
const err31 = {instancePath:instancePath+"/items/" + i0+"/outcome",schemaPath:"#/properties/items/items/properties/outcome/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
else {
errors = _errs18;
if(vErrors !== null){
if(_errs18){
vErrors.length = _errs18;
}
else {
vErrors = null;
}
}
}
}
if(data3.selected !== undefined){
if(typeof data3.selected !== "boolean"){
const err32 = {instancePath:instancePath+"/items/" + i0+"/selected",schemaPath:"#/properties/items/items/properties/selected/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
}
if(data3.source !== undefined){
let data9 = data3.source;
if(Array.isArray(data9)){
const len1 = data9.length;
for(let i1=0; i1<len1; i1++){
if(typeof data9[i1] !== "string"){
const err33 = {instancePath:instancePath+"/items/" + i0+"/source/" + i1,schemaPath:"#/properties/items/items/properties/source/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
}
}
else {
const err34 = {instancePath:instancePath+"/items/" + i0+"/source",schemaPath:"#/properties/items/items/properties/source/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
}
if(data3.target !== undefined){
let data11 = data3.target;
if(Array.isArray(data11)){
const len2 = data11.length;
for(let i2=0; i2<len2; i2++){
if(typeof data11[i2] !== "string"){
const err35 = {instancePath:instancePath+"/items/" + i0+"/target/" + i2,schemaPath:"#/properties/items/items/properties/target/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
}
}
else {
const err36 = {instancePath:instancePath+"/items/" + i0+"/target",schemaPath:"#/properties/items/items/properties/target/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
}
}
}
else {
const err37 = {instancePath:instancePath+"/items/" + i0,schemaPath:"#/properties/items/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err37];
}
else {
vErrors.push(err37);
}
errors++;
}
}
}
else {
const err38 = {instancePath:instancePath+"/items",schemaPath:"#/properties/items/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err38];
}
else {
vErrors.push(err38);
}
errors++;
}
}
if(data.originalRunId !== undefined){
if(typeof data.originalRunId !== "string"){
const err39 = {instancePath:instancePath+"/originalRunId",schemaPath:"#/properties/originalRunId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err39];
}
else {
vErrors.push(err39);
}
errors++;
}
}
if(data.reverted !== undefined){
let data14 = data.reverted;
if(!(((typeof data14 == "number") && (!(data14 % 1) && !isNaN(data14))) && (isFinite(data14)))){
const err40 = {instancePath:instancePath+"/reverted",schemaPath:"#/properties/reverted/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err40];
}
else {
vErrors.push(err40);
}
errors++;
}
if((typeof data14 == "number") && (isFinite(data14))){
if(data14 < 0 || isNaN(data14)){
const err41 = {instancePath:instancePath+"/reverted",schemaPath:"#/properties/reverted/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err41];
}
else {
vErrors.push(err41);
}
errors++;
}
}
}
if(data.runId !== undefined){
if(typeof data.runId !== "string"){
const err42 = {instancePath:instancePath+"/runId",schemaPath:"#/properties/runId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err42];
}
else {
vErrors.push(err42);
}
errors++;
}
}
if(data.status !== undefined){
let data16 = data.status;
if(typeof data16 !== "string"){
const err43 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err43];
}
else {
vErrors.push(err43);
}
errors++;
}
if(!(((((((data16 === "queued") || (data16 === "running")) || (data16 === "completed")) || (data16 === "partial")) || (data16 === "failed")) || (data16 === "cancelled")) || (data16 === "recoveryRequired"))){
const err44 = {instancePath:instancePath+"/status",schemaPath:"#/properties/status/enum",keyword:"enum",params:{allowedValues: schema39.properties.status.enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err44];
}
else {
vErrors.push(err44);
}
errors++;
}
}
if(data.untouched !== undefined){
let data17 = data.untouched;
if(!(((typeof data17 == "number") && (!(data17 % 1) && !isNaN(data17))) && (isFinite(data17)))){
const err45 = {instancePath:instancePath+"/untouched",schemaPath:"#/properties/untouched/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err45];
}
else {
vErrors.push(err45);
}
errors++;
}
if((typeof data17 == "number") && (isFinite(data17))){
if(data17 < 0 || isNaN(data17)){
const err46 = {instancePath:instancePath+"/untouched",schemaPath:"#/properties/untouched/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err46];
}
else {
vErrors.push(err46);
}
errors++;
}
}
}
if(data.warnings !== undefined){
let data18 = data.warnings;
if(Array.isArray(data18)){
const len3 = data18.length;
for(let i3=0; i3<len3; i3++){
let data19 = data18[i3];
if(data19 && typeof data19 == "object" && !Array.isArray(data19)){
if(data19.code === undefined){
const err47 = {instancePath:instancePath+"/warnings/" + i3,schemaPath:"#/properties/warnings/items/required",keyword:"required",params:{missingProperty: "code"},message:"must have required property '"+"code"+"'"};
if(vErrors === null){
vErrors = [err47];
}
else {
vErrors.push(err47);
}
errors++;
}
if(data19.severity === undefined){
const err48 = {instancePath:instancePath+"/warnings/" + i3,schemaPath:"#/properties/warnings/items/required",keyword:"required",params:{missingProperty: "severity"},message:"must have required property '"+"severity"+"'"};
if(vErrors === null){
vErrors = [err48];
}
else {
vErrors.push(err48);
}
errors++;
}
if(data19.itemId === undefined){
const err49 = {instancePath:instancePath+"/warnings/" + i3,schemaPath:"#/properties/warnings/items/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err49];
}
else {
vErrors.push(err49);
}
errors++;
}
if(data19.message === undefined){
const err50 = {instancePath:instancePath+"/warnings/" + i3,schemaPath:"#/properties/warnings/items/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err50];
}
else {
vErrors.push(err50);
}
errors++;
}
for(const key2 in data19){
if(!((((key2 === "code") || (key2 === "itemId")) || (key2 === "message")) || (key2 === "severity"))){
const err51 = {instancePath:instancePath+"/warnings/" + i3,schemaPath:"#/properties/warnings/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err51];
}
else {
vErrors.push(err51);
}
errors++;
}
}
if(data19.code !== undefined){
if(typeof data19.code !== "string"){
const err52 = {instancePath:instancePath+"/warnings/" + i3+"/code",schemaPath:"#/properties/warnings/items/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err52];
}
else {
vErrors.push(err52);
}
errors++;
}
}
if(data19.itemId !== undefined){
let data21 = data19.itemId;
if((typeof data21 !== "string") && (data21 !== null)){
const err53 = {instancePath:instancePath+"/warnings/" + i3+"/itemId",schemaPath:"#/properties/warnings/items/properties/itemId/type",keyword:"type",params:{type: schema39.properties.warnings.items.properties.itemId.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err53];
}
else {
vErrors.push(err53);
}
errors++;
}
}
if(data19.message !== undefined){
if(typeof data19.message !== "string"){
const err54 = {instancePath:instancePath+"/warnings/" + i3+"/message",schemaPath:"#/properties/warnings/items/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err54];
}
else {
vErrors.push(err54);
}
errors++;
}
}
if(data19.severity !== undefined){
let data23 = data19.severity;
const _errs57 = errors;
let valid12 = false;
let passing1 = null;
const _errs58 = errors;
if(typeof data23 !== "string"){
const err55 = {instancePath:instancePath+"/warnings/" + i3+"/severity",schemaPath:"#/properties/warnings/items/properties/severity/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err55];
}
else {
vErrors.push(err55);
}
errors++;
}
if(!((data23 === "info") || (data23 === "warning"))){
const err56 = {instancePath:instancePath+"/warnings/" + i3+"/severity",schemaPath:"#/properties/warnings/items/properties/severity/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema39.properties.warnings.items.properties.severity.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err56];
}
else {
vErrors.push(err56);
}
errors++;
}
var _valid1 = _errs58 === errors;
if(_valid1){
valid12 = true;
passing1 = 0;
}
const _errs60 = errors;
if(typeof data23 !== "string"){
const err57 = {instancePath:instancePath+"/warnings/" + i3+"/severity",schemaPath:"#/properties/warnings/items/properties/severity/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err57];
}
else {
vErrors.push(err57);
}
errors++;
}
if("block" !== data23){
const err58 = {instancePath:instancePath+"/warnings/" + i3+"/severity",schemaPath:"#/properties/warnings/items/properties/severity/oneOf/1/const",keyword:"const",params:{allowedValue: "block"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err58];
}
else {
vErrors.push(err58);
}
errors++;
}
var _valid1 = _errs60 === errors;
if(_valid1 && valid12){
valid12 = false;
passing1 = [passing1, 1];
}
else {
if(_valid1){
valid12 = true;
passing1 = 1;
}
}
if(!valid12){
const err59 = {instancePath:instancePath+"/warnings/" + i3+"/severity",schemaPath:"#/properties/warnings/items/properties/severity/oneOf",keyword:"oneOf",params:{passingSchemas: passing1},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err59];
}
else {
vErrors.push(err59);
}
errors++;
}
else {
errors = _errs57;
if(vErrors !== null){
if(_errs57){
vErrors.length = _errs57;
}
else {
vErrors = null;
}
}
}
}
}
else {
const err60 = {instancePath:instancePath+"/warnings/" + i3,schemaPath:"#/properties/warnings/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err60];
}
else {
vErrors.push(err60);
}
errors++;
}
}
}
else {
const err61 = {instancePath:instancePath+"/warnings",schemaPath:"#/properties/warnings/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err61];
}
else {
vErrors.push(err61);
}
errors++;
}
}
}
else {
const err62 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err62];
}
else {
vErrors.push(err62);
}
errors++;
}
validate38.errors = vErrors;
return errors === 0;
}

export const ValidationReport = validate39;
const schema40 = {"additionalProperties":false,"description":"校验报告。","properties":{"digest":{"description":"由后端对固定字段的规范序列化结果计算 SHA-256。","type":"string"},"executableCount":{"format":"uint32","minimum":0,"type":"integer"},"expiresAt":{"description":"UTC RFC3339，最多 5 分钟有效。","type":["string","null"]},"issues":{"items":{"additionalProperties":false,"description":"校验或执行过程中的一个问题。","properties":{"code":{"type":"string"},"itemId":{"description":"与具体计划项相关时给出；全局问题为 `None`。","type":["string","null"]},"message":{"type":"string"},"severity":{"description":"校验问题的严重度。","oneOf":[{"enum":["info","warning"],"type":"string"},{"const":"block","description":"阻断项：只要存在，就不能生成可执行计划。","type":"string"}]}},"required":["code","severity","itemId","message"],"type":"object"},"type":"array"},"planId":{"type":"string"},"revision":{"description":"该报告对应的计划版本。与当前计划版本不一致即视为过期。","format":"uint32","minimum":0,"type":"integer"},"validationToken":{"description":"一次性令牌。**只有**后端能生成，前端不得自行生成或伪造。","type":["string","null"]}},"required":["planId","revision","digest","executableCount","issues","validationToken","expiresAt"],"type":"object"};

function validate39(data, {instancePath="", parentData, parentDataProperty, rootData=data}={}){
let vErrors = null;
let errors = 0;
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.planId === undefined){
const err0 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "planId"},message:"must have required property '"+"planId"+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if(data.revision === undefined){
const err1 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "revision"},message:"must have required property '"+"revision"+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if(data.digest === undefined){
const err2 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "digest"},message:"must have required property '"+"digest"+"'"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(data.executableCount === undefined){
const err3 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "executableCount"},message:"must have required property '"+"executableCount"+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if(data.issues === undefined){
const err4 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "issues"},message:"must have required property '"+"issues"+"'"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if(data.validationToken === undefined){
const err5 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "validationToken"},message:"must have required property '"+"validationToken"+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
if(data.expiresAt === undefined){
const err6 = {instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: "expiresAt"},message:"must have required property '"+"expiresAt"+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
for(const key0 in data){
if(!(((((((key0 === "digest") || (key0 === "executableCount")) || (key0 === "expiresAt")) || (key0 === "issues")) || (key0 === "planId")) || (key0 === "revision")) || (key0 === "validationToken"))){
const err7 = {instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
if(data.digest !== undefined){
if(typeof data.digest !== "string"){
const err8 = {instancePath:instancePath+"/digest",schemaPath:"#/properties/digest/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
if(data.executableCount !== undefined){
let data1 = data.executableCount;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err9 = {instancePath:instancePath+"/executableCount",schemaPath:"#/properties/executableCount/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err10 = {instancePath:instancePath+"/executableCount",schemaPath:"#/properties/executableCount/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
}
if(data.expiresAt !== undefined){
let data2 = data.expiresAt;
if((typeof data2 !== "string") && (data2 !== null)){
const err11 = {instancePath:instancePath+"/expiresAt",schemaPath:"#/properties/expiresAt/type",keyword:"type",params:{type: schema40.properties.expiresAt.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
}
if(data.issues !== undefined){
let data3 = data.issues;
if(Array.isArray(data3)){
const len0 = data3.length;
for(let i0=0; i0<len0; i0++){
let data4 = data3[i0];
if(data4 && typeof data4 == "object" && !Array.isArray(data4)){
if(data4.code === undefined){
const err12 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "code"},message:"must have required property '"+"code"+"'"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if(data4.severity === undefined){
const err13 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "severity"},message:"must have required property '"+"severity"+"'"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
if(data4.itemId === undefined){
const err14 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "itemId"},message:"must have required property '"+"itemId"+"'"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
if(data4.message === undefined){
const err15 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/required",keyword:"required",params:{missingProperty: "message"},message:"must have required property '"+"message"+"'"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
for(const key1 in data4){
if(!((((key1 === "code") || (key1 === "itemId")) || (key1 === "message")) || (key1 === "severity"))){
const err16 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
if(data4.code !== undefined){
if(typeof data4.code !== "string"){
const err17 = {instancePath:instancePath+"/issues/" + i0+"/code",schemaPath:"#/properties/issues/items/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
}
if(data4.itemId !== undefined){
let data6 = data4.itemId;
if((typeof data6 !== "string") && (data6 !== null)){
const err18 = {instancePath:instancePath+"/issues/" + i0+"/itemId",schemaPath:"#/properties/issues/items/properties/itemId/type",keyword:"type",params:{type: schema40.properties.issues.items.properties.itemId.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
}
if(data4.message !== undefined){
if(typeof data4.message !== "string"){
const err19 = {instancePath:instancePath+"/issues/" + i0+"/message",schemaPath:"#/properties/issues/items/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
}
if(data4.severity !== undefined){
let data8 = data4.severity;
const _errs20 = errors;
let valid4 = false;
let passing0 = null;
const _errs21 = errors;
if(typeof data8 !== "string"){
const err20 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
if(!((data8 === "info") || (data8 === "warning"))){
const err21 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/0/enum",keyword:"enum",params:{allowedValues: schema40.properties.issues.items.properties.severity.oneOf[0].enum},message:"must be equal to one of the allowed values"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
var _valid0 = _errs21 === errors;
if(_valid0){
valid4 = true;
passing0 = 0;
}
const _errs23 = errors;
if(typeof data8 !== "string"){
const err22 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
if("block" !== data8){
const err23 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf/1/const",keyword:"const",params:{allowedValue: "block"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
var _valid0 = _errs23 === errors;
if(_valid0 && valid4){
valid4 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid4 = true;
passing0 = 1;
}
}
if(!valid4){
const err24 = {instancePath:instancePath+"/issues/" + i0+"/severity",schemaPath:"#/properties/issues/items/properties/severity/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
else {
errors = _errs20;
if(vErrors !== null){
if(_errs20){
vErrors.length = _errs20;
}
else {
vErrors = null;
}
}
}
}
}
else {
const err25 = {instancePath:instancePath+"/issues/" + i0,schemaPath:"#/properties/issues/items/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
}
else {
const err26 = {instancePath:instancePath+"/issues",schemaPath:"#/properties/issues/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
if(data.planId !== undefined){
if(typeof data.planId !== "string"){
const err27 = {instancePath:instancePath+"/planId",schemaPath:"#/properties/planId/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
}
if(data.revision !== undefined){
let data10 = data.revision;
if(!(((typeof data10 == "number") && (!(data10 % 1) && !isNaN(data10))) && (isFinite(data10)))){
const err28 = {instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
if((typeof data10 == "number") && (isFinite(data10))){
if(data10 < 0 || isNaN(data10)){
const err29 = {instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
}
}
}
if(data.validationToken !== undefined){
let data11 = data.validationToken;
if((typeof data11 !== "string") && (data11 !== null)){
const err30 = {instancePath:instancePath+"/validationToken",schemaPath:"#/properties/validationToken/type",keyword:"type",params:{type: schema40.properties.validationToken.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
}
}
else {
const err31 = {instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
validate39.errors = vErrors;
return errors === 0;
}
