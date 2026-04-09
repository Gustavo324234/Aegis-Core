import { useState } from 'react';
import AegisFullLogo from '@/assets/branding/aegis_logo.svg';
import AegisIconLogo from '@/assets/branding/logo_icon.svg';

export interface AegisLogoProps {
    variant?: 'full' | 'icon';
    className?: string;
}

export function AegisLogo({ variant = 'icon', className = '' }: AegisLogoProps) {
    const [hasError, setHasError] = useState(false);

    // Si hubo error al cargar o si no están disponibles los componentes, fallback a AEGIS
    if (hasError) {
        return (
            <span className={`font-mono font-bold tracking-wider ${className}`}>
                AEGIS
            </span>
        );
    }

    const logoSrc = variant === 'full' ? AegisFullLogo : AegisIconLogo;

    return (
        <div className={`flex items-center justify-center ${className}`}>
            <img 
                src={logoSrc} 
                alt="Aegis Logo" 
                className="w-full h-full object-contain"
                onError={() => setHasError(true)}
            />
        </div>
    );
}
