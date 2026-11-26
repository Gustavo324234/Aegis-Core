import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { AegisLogo } from '../components/AegisLogo';

describe('AegisLogo', () => {
    it('renders icon and full logo variants using img element', () => {
        const { rerender } = render(<AegisLogo variant="icon" className="w-6 h-6" />);
        const iconImg = screen.getByAltText('Aegis Logo') as HTMLImageElement;
        expect(iconImg).toBeInTheDocument();
        expect(iconImg.src).toBeTruthy();

        rerender(<AegisLogo variant="full" className="w-24" />);
        const fullImg = screen.getByAltText('Aegis Logo') as HTMLImageElement;
        expect(fullImg).toBeInTheDocument();
        expect(fullImg.src).not.toBe('');
    });

    it('falls back to AEGIS text badge when image fails to load', () => {
        render(<AegisLogo variant="icon" className="w-6 h-6" />);
        const img = screen.getByAltText('Aegis Logo');
        fireEvent.error(img);

        expect(screen.queryByAltText('Aegis Logo')).not.toBeInTheDocument();
        expect(screen.getByText('AEGIS')).toBeInTheDocument();
    });
});
