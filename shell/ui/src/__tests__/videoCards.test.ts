import { describe, expect, it } from 'vitest';
import { extractVideoCardsFromMessage } from '../utils/videoCards';

describe('extractVideoCardsFromMessage', () => {
    it('extracts and deduplicates [AEGIS_VIDEO_CARD:id] tags while stripping them from content', () => {
        const raw = 'Aquí tenés tu video:\n[AEGIS_VIDEO_CARD:vid-abc123]\n[AEGIS_VIDEO_CARD:vid-abc123]\nListo para descargar.';
        const result = extractVideoCardsFromMessage(raw, true);

        expect(result.videoProjectIds).toEqual(['vid-abc123']);
        expect(result.cleanContent).toBe('Aquí tenés tu video:\n\n\nListo para descargar.');
    });

    it('extracts multiple distinct video project IDs in order', () => {
        const raw = '[AEGIS_VIDEO_CARD:vid-first_01] Compará con [AEGIS_VIDEO_CARD:vid-second_02]';
        const result = extractVideoCardsFromMessage(raw, false);

        expect(result.videoProjectIds).toEqual(['vid-first_01', 'vid-second_02']);
        expect(result.cleanContent).toBe('Compará con');
    });

    it('ignores malformed tags and short IDs', () => {
        const raw = 'Texto normal [AEGIS_VIDEO_CARD:ab] y [AEGIS_VIDEO_CARD:bad space!]';
        const result = extractVideoCardsFromMessage(raw, true);

        expect(result.videoProjectIds).toEqual([]);
        expect(result.cleanContent).toBe(raw);
    });

    it('detects legacy vid-* IDs only in assistant messages mentioning output.mp4 or renderizado', () => {
        const assistantMsg = 'El proyecto vid-12ab34cd fue renderizado con éxito.';
        expect(extractVideoCardsFromMessage(assistantMsg, true).videoProjectIds).toEqual(['vid-12ab34cd']);

        expect(extractVideoCardsFromMessage(assistantMsg, false).videoProjectIds).toEqual([]);
        expect(
            extractVideoCardsFromMessage('Proyecto vid-12ab34cd mencionado sin render.', true).videoProjectIds,
        ).toEqual([]);
    });
});
