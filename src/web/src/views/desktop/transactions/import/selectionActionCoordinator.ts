export interface SelectionSnapshotRow {
    id: number;
    baselineSelected: boolean;
    selected: boolean;
}

export function buildSelectionPatch(rows: SelectionSnapshotRow[]): {
    selectedIds: number[];
    deselectedIds: number[];
} {
    return rows.reduce((patch, row) => {
        if (row.selected !== row.baselineSelected) {
            (row.selected ? patch.selectedIds : patch.deselectedIds).push(row.id);
        }
        return patch;
    }, { selectedIds: [] as number[], deselectedIds: [] as number[] });
}
