import { spawnSync } from 'child_process';

import { expect, test, type Page } from '@playwright/test';

import {
    createImportSignalFixtureContext,
    IMPORT_SIGNAL_MARKERS,
    openImportSignalDialog,
    patchImportSignalPreview,
    refreshPreviewThroughFilter,
    type ImportSignalDedupResponse,
    type ImportSignalFixtureContext,
    type ImportSignalPreviewPage
} from '../helpers/importSignalSystem';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

type Session = Awaited<ReturnType<typeof createCleanE2ESession>>;

test.describe('G019 import preview visible UI contracts', () => {
    test.describe.configure({ mode: 'serial', retries: 0, timeout: 120_000 });

    test('history hover is readable and opens the real bill detail', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const fixture = await stageSignalFixture(page, session);
            const history = seedHistoryBillAndPreview(fixture.sessionId, fixture.context);
            await refreshPreviewThroughFilter(page, 'History Rewrite');

            const table = page.getByTestId('desktop.import.preview.table');
            const historyRow = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.history });
            const historyChip = historyRow.getByText('History Rewrite', { exact: true });
            expect(await historyChip.count()).toBe(1);
            await historyChip.hover();

            await expect(page.getByText('分类：G019 History Category', { exact: true })).toBeVisible();
            await expect(page.getByText('账户：G019 History Account', { exact: true })).toBeVisible();
            await expect(page.getByText('对方：G019 History Merchant', { exact: true })).toBeVisible();
            await expect(page.getByText('Operation: update_history', { exact: true })).toHaveCount(0);
            await expect(page.getByText(`History Bill: #${history.billId} v1`, { exact: true })).toHaveCount(0);

            const detailResponsePromise = page.waitForResponse(response => {
                const url = new URL(response.url());
                return response.request().method() === 'GET'
                    && url.pathname === '/api/bills/get'
                    && url.searchParams.get('id') === String(history.billId);
            });
            await historyChip.click();
            expect((await detailResponsePromise).ok(), 'real history bill detail route').toBe(true);
            const detailDialog = page.getByRole('dialog').filter({ hasText: 'Transaction Detail' });
            await expect(detailDialog.getByPlaceholder('Expense Amount')).toHaveValue('18.80');
            await expect(detailDialog.getByPlaceholder('Your transaction description (optional)'))
                .toHaveValue('G019 readable history detail');
            await page.screenshot({ path: evidencePath('g019-history-detail.png'), fullPage: true });
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('three visible actions send the same cross-page selected scope', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const fixture = await stageSignalFixture(page, session);
            const selectedIds = await selectThreeAcrossPages(session, fixture.sessionId);
            const metadata = await refreshPreviewThroughFilter(page, 'Parser');
            const selectionHash = readSelectionHash(metadata);
            expect(selectionHash).not.toBe('');

            const captured = await clickThreeActions(page, fixture.sessionId);
            for (const payload of captured) {
                expect(payload['action_scope']).toEqual({ kind: 'selected', selection_hash: selectionHash });
            }
            expect(selectedIds).toHaveLength(3);
            await page.screenshot({ path: evidencePath('g019-selected-actions.png'), fullPage: true });
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('three visible actions fall back to current all-matching filters after select-none', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const fixture = await stageSignalFixture(page, session);
            await session.client.put(`bills/import/v2/preview/${encodeURIComponent(fixture.sessionId)}/selection`, {
                selectionAction: 'select_none'
            });
            await refreshPreviewThroughFilter(page, 'Parser');

            const captured = await clickThreeActions(page, fixture.sessionId);
            const scopes = captured.map(payload => payload['action_scope'] as Record<string, unknown>);
            expect(scopes[1]).toEqual(scopes[0]);
            expect(scopes[2]).toEqual(scopes[0]);
            for (const scope of scopes) {
                expect(scope).toMatchObject({
                    kind: 'all_matching',
                    filters: { signal: 'parser', selected_only: false }
                });
                expect(String(scope['filter_hash'] || '')).not.toBe('');
            }
        } finally {
            await cleanupE2ESession(session);
        }
    });
});

async function stageSignalFixture(page: Page, session: Session): Promise<{
    sessionId: string;
    context: ImportSignalFixtureContext;
}> {
    const context = await createImportSignalFixtureContext(session.client, session.env);
    await openImportSignalDialog(page, session.env);
    const dedupPromise = page.waitForResponse(response => (
        response.request().method() === 'POST'
        && response.url().includes('/api/bills/import/v2/dedup')
    ));
    await page.getByTestId('desktop.import.action.next').click();
    const response = await dedupPromise;
    expect(response.ok()).toBe(true);
    const envelope = await response.json() as { data?: ImportSignalDedupResponse };
    const sessionId = String(envelope.data?.session_id || '');
    expect(sessionId).not.toBe('');
    await expect(page.getByTestId('desktop.import.preview.table')).toBeVisible();
    patchImportSignalPreview(sessionId, context);
    return { sessionId, context };
}

function seedHistoryBillAndPreview(sessionId: string, context: ImportSignalFixtureContext): { billId: number } {
    const accountId = Number(context.account.id);
    const categoryId = Number(context.expenseCategory.id);
    const sql = `
WITH target_session AS (
    SELECT id, user_id FROM import_sessions WHERE session_key = ${sqlLiteral(sessionId)}
), inserted AS (
    INSERT INTO bills (
        user_id, occurred_at, amount_cents, direction, transaction_type,
        account_id, source_account_id, category_id, merchant, payment_method,
        description, parser_name, source_hash
    )
    SELECT user_id, '2026-07-01 09:30:00+08', 1880, 'expense', 'expense',
        ${accountId}, ${accountId}, ${categoryId}, 'G019 History Merchant', 'card',
        'G019 readable history detail', 'g019-history', ${sqlLiteral(`g019-history-${sessionId}`)}
    FROM target_session
    RETURNING id, version
), updated AS (
    UPDATE import_preview_rows p
    SET preview_payload = p.preview_payload || jsonb_build_object(
        'preview_matching_feedback', jsonb_build_object(
            'reconciliation', jsonb_build_object(
                'review_status', 'pending',
                'planned_operation', 'update_history',
                'history_bill_id', inserted.id,
                'history_bill_version', inserted.version,
                'operation_id', 'g019-history-operation',
                'acknowledgement_token', 'g019-history-ack',
                'destructive_ack_required', true,
                'notice', '将改写/合并历史账单',
                'history_summary', jsonb_build_object(
                    'bill_id', inserted.id,
                    'date_time', '2026-07-01 09:30:00',
                    'amount_cents', -1880,
                    'currency', 'CNY',
                    'category_name', 'G019 History Category',
                    'source_account_name', 'G019 History Account',
                    'counterparty', 'G019 History Merchant',
                    'description', 'G019 readable history detail'
                )
            )
        )
    )
    FROM target_session, inserted
    WHERE p.session_id = target_session.id
      AND p.merchant = ${sqlLiteral(IMPORT_SIGNAL_MARKERS.history)}
    RETURNING inserted.id
)
SELECT id FROM updated;
`;
    return { billId: Number(runPsql(sql).trim()) };
}

async function selectThreeAcrossPages(session: Session, sessionId: string): Promise<number[]> {
    const first = await session.client.get<ImportSignalPreviewPage>(
        `bills/import/v2/preview/${encodeURIComponent(sessionId)}?page=1&page_size=10&sort_by=time&sort_direction=asc`
    );
    const second = await session.client.get<ImportSignalPreviewPage>(
        `bills/import/v2/preview/${encodeURIComponent(sessionId)}?page=2&page_size=10&sort_by=time&sort_direction=asc`
    );
    const selectedIds = [
        Number(first.preview?.[0]?.['id']),
        Number(first.preview?.[1]?.['id']),
        Number(second.preview?.[0]?.['id'])
    ];
    expect(selectedIds.every(id => id > 0)).toBe(true);
    await session.client.put(`bills/import/v2/preview/${encodeURIComponent(sessionId)}/selection`, {
        selectionAction: 'select_none'
    });
    await session.client.put(`bills/import/v2/preview/${encodeURIComponent(sessionId)}/selection`, {
        selectionAction: 'patch',
        selectedIds,
        deselectedIds: []
    });
    return selectedIds;
}

async function clickThreeActions(page: Page, sessionId: string): Promise<Array<Record<string, unknown>>> {
    const captured: Array<Record<string, unknown>> = [];
    await page.route('**/api/llm/preview-recommend', async route => {
        captured.push(route.request().postDataJSON() as Record<string, unknown>);
        await route.fulfill({ json: { success: true, data: { result: { suggestions: [] } } } });
    });
    await page.route('**/api/llm/analyze-transactions', async route => {
        captured.push(route.request().postDataJSON() as Record<string, unknown>);
        await route.fulfill({ json: { success: true, data: { result: { candidates_created: 0 } } } });
    });
    await page.route(`**/api/bills/import/v2/learning/${encodeURIComponent(sessionId)}/suggestions`, async route => {
        captured.push(route.request().postDataJSON() as Record<string, unknown>);
        await route.fulfill({ json: { success: true, data: { result: { suggestions: [] } } } });
    });

    for (const name of [
        'Apply LLM Suggestions',
        'Generate LLM Rule Candidates',
        'Save as Long-term Learning'
    ]) {
        const button = page.getByRole('button', { name, exact: true });
        expect(await button.count()).toBe(1);
        await expect(button).toBeEnabled();
        await button.click();
    }
    expect(captured).toHaveLength(3);
    return captured;
}

function readSelectionHash(envelope: Record<string, unknown>): string {
    const data = envelope['data'] as Record<string, unknown> | undefined;
    const metadata = data?.['metadata'] as Record<string, unknown> | undefined;
    return String(metadata?.['selection_hash'] || '');
}

function runPsql(sql: string): string {
    const result = spawnSync('docker', [
        'exec', '-i', process.env['E2E_POSTGRES_CONTAINER'] || 'bill-analyser-postgres',
        'psql', '-U', 'bill_analyser', '-d', process.env['E2E_POSTGRES_DATABASE'] || 'bill_analyser_e2e_g019',
        '-v', 'ON_ERROR_STOP=1', '-q', '-A', '-t'
    ], { input: sql, encoding: 'utf8', windowsHide: true });
    if (result.status !== 0) throw new Error(result.stderr.trim());
    return result.stdout;
}

function sqlLiteral(value: string): string {
    return `'${value.replace(/'/gu, "''")}'`;
}

function evidencePath(name: string): string {
    return `C:/Users/hcy/OneDrive/Github/bill_analyser/.omx/context/${name}`;
}
