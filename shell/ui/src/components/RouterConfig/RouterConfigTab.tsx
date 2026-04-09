import React from 'react';
import { useAegisStore } from '../../store/useAegisStore';
import GlobalKeyManager from './GlobalKeyManager';
import TenantKeyManager from './TenantKeyManager';
import ModelCatalogViewer from './ModelCatalogViewer';

const RouterConfigTab: React.FC = () => {
    const { isAdmin, tenantId, sessionKey } = useAegisStore();

    if (!tenantId || !sessionKey) return null;

    return (
        <div className="space-y-8">
            {isAdmin ? (
                <GlobalKeyManager tenantId={tenantId} sessionKey={sessionKey} />
            ) : (
                <TenantKeyManager tenantId={tenantId} sessionKey={sessionKey} />
            )}
            <ModelCatalogViewer tenantId={tenantId} sessionKey={sessionKey} isAdmin={isAdmin} />
        </div>
    );
};

export default RouterConfigTab;
