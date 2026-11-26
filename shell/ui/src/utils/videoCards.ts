const VIDEO_CARD_TAG_RE = /\[AEGIS_VIDEO_CARD:([A-Za-z0-9_-]{4,40})\]/g;
const LEGACY_VIDEO_ID_RE = /\b(vid-[a-f0-9]{6,16})\b/gi;

export interface ExtractedVideoCards {
    cleanContent: string;
    videoProjectIds: string[];
}

export function extractVideoCardsFromMessage(
    content: string,
    isAssistant: boolean,
): ExtractedVideoCards {
    const ids: string[] = [];
    const seen = new Set<string>();
    const addId = (id: string) => {
        if (!seen.has(id)) {
            seen.add(id);
            ids.push(id);
        }
    };

    const cleanContent = content.replace(VIDEO_CARD_TAG_RE, (_m, pid: string) => {
        addId(pid);
        return '';
    }).trim();

    if (isAssistant && ids.length === 0 && (content.includes('output.mp4') || /renderizad[oa]/i.test(content))) {
        let match: RegExpExecArray | null;
        const re = new RegExp(LEGACY_VIDEO_ID_RE.source, 'gi');
        while ((match = re.exec(content)) !== null) {
            addId(match[1]);
        }
    }

    return { cleanContent, videoProjectIds: ids };
}
