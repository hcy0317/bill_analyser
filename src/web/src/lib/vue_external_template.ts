// 中文导读：外置 Vue template 的类型检查辅助。
// 维护重点：template 使用 `src` 外置时，vue-tsc 不会把模板内绑定计为 script 使用；这里用显式 no-op 标记绑定合同。
export function useExternalTemplateBindings(..._bindings: unknown[]): void {
    // no-op: 仅服务于 TypeScript noUnusedLocals，不参与运行态状态。
}
