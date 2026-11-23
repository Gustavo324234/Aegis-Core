import { useState, useEffect } from 'react';

// Fetches protected /api/video/media/* using Citadel auth headers cleanly without leaking credentials in URLs.
export const useAuthenticatedBlobUrl = (
    url: string | null,
    tenantId: string | null,
    sessionKey: string | null,
    reloadToken: string | number,
) => {
    const [blobUrl, setBlobUrl] = useState<string | null>(null);

    useEffect(() => {
        if (!url || !tenantId || !sessionKey) {
            setBlobUrl(null);
            return;
        }
        let cancelled = false;
        let objectUrl: string | null = null;

        fetch(url, {
            headers: {
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
            },
        })
            .then((res) => (res.ok ? res.blob() : null))
            .then((blob) => {
                if (!cancelled && blob) {
                    objectUrl = URL.createObjectURL(blob);
                    setBlobUrl(objectUrl);
                }
            })
            .catch(() => {
                if (!cancelled) setBlobUrl(null);
            });

        return () => {
            cancelled = true;
            if (objectUrl) {
                URL.revokeObjectURL(objectUrl);
            }
        };
    }, [url, tenantId, sessionKey, reloadToken]);

    return blobUrl;
};
