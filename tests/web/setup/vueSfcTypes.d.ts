declare module '*.vue' {
    export type BatchCreateDialogDataType =
        | 'expenseCategory'
        | 'incomeCategory'
        | 'transferCategory'
        | 'tag';

    export type BatchReplaceDialogDataType =
        | BatchCreateDialogDataType
        | 'account'
        | 'destinationAccount';

    export type BatchReplaceDialogMode =
        | 'batchReplace'
        | 'batchAdd'
        | 'replaceInvalidItems';

    interface VueSfcPublicInstance {
        open(...args: any[]): Promise<Record<string, any> | null | undefined>;
        showError(...args: any[]): void;
        showMessage(...args: any[]): void;
    }

    const component: new (...args: never[]) => VueSfcPublicInstance;
    export default component;
}
