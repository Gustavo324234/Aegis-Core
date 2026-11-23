export interface ExposedToolSchema {
    name: string;
    description: string;
}

export interface ModuleSummary {
    module_id: string;
    display_name: string;
    version: string;
    active: boolean;
    exposed_tools: ExposedToolSchema[];
}

export interface CreatorIdea {
    id: string;
    title: string;
    platform: string;
    hook: string;
    pillar: string;
    notes: string;
    status: string;
    priority: string;
    updated_at?: string;
}

export interface CreatorScript {
    id: string;
    idea_id?: string;
    title: string;
    platform: string;
    hook_0_3s: string;
    body_script: string;
    b_roll_notes: string;
    cta: string;
    caption: string;
    hashtags: string;
    estimated_duration_secs?: number;
    status: string;
}

export interface CreatorPost {
    id: string;
    title: string;
    platform: string;
    publish_at: string;
    caption: string;
    hashtags: string;
    status: string;
}

export interface CreatorOverview {
    ideas: CreatorIdea[];
    scripts: CreatorScript[];
    calendar: CreatorPost[];
    stats: {
        total_ideas: number;
        total_scripts: number;
        ready_to_film: number;
        scheduled_posts: number;
    };
}

export interface DevProject {
    id: string;
    name: string;
    repo_path: string;
    stack: string;
    description: string;
    default_branch: string;
    build_cmd: string;
    test_cmd: string;
    status: string;
}

export interface DevTask {
    id: string;
    project_id: string;
    title: string;
    kind: string;
    priority: string;
    status: string;
    file_refs: string;
    notes: string;
}

export interface DevSnippet {
    id: string;
    title: string;
    language: string;
    code: string;
    tags: string;
    description: string;
}

export interface DevOverview {
    projects: DevProject[];
    tasks: DevTask[];
    snippets: DevSnippet[];
    stats: {
        total_projects: number;
        open_tasks: number;
        in_progress_tasks: number;
        bugs_count: number;
        total_snippets: number;
    };
}

export interface BizProduct {
    id: string;
    barcode: string;
    name: string;
    category: string;
    price: number;
    cost: number;
    stock: number;
    min_stock: number;
}

export interface BizSale {
    id: string;
    product_name: string;
    quantity: number;
    unit_price: number;
    total: number;
    customer: string;
    payment_method: string;
    created_at: string;
}

export interface BizOverview {
    products: BizProduct[];
    sales: BizSale[];
    stats: {
        total_products: number;
        low_stock_count: number;
        inventory_value: number;
        total_sales_count: number;
        total_revenue: number;
    };
}

export interface ModulesAppHubProps {
    isOpen: boolean;
    onClose: () => void;
    initialView?: string;
    initialVideoProjectId?: string | null;
    onConsumeInitialVideoProject?: () => void;
}

export const PLATFORM_LABELS: Record<string, string> = {
    tiktok_reels: 'TikTok / Reels',
    youtube: 'YouTube',
    shorts: 'YT Shorts',
    instagram: 'Instagram',
    x_twitter: 'X / Twitter',
    linkedin: 'LinkedIn',
};

export const IDEA_STAGES = ['idea', 'scripting', 'filming', 'editing', 'scheduled', 'published'] as const;
export const IDEA_STAGE_LABELS: Record<string, string> = {
    idea: 'Idea',
    scripting: 'Guion',
    filming: 'Grabando',
    editing: 'Editando',
    scheduled: 'Programado',
    published: 'Publicado',
};

export const DEV_COLUMNS = ['backlog', 'in_progress', 'review', 'done'] as const;
export const DEV_COLUMN_LABELS: Record<string, string> = {
    backlog: 'Backlog',
    in_progress: 'En Curso',
    review: 'Review',
    done: 'Hecho',
};

export function moduleDescription(moduleId: string): { tagline: string; highlights: string[] } {
    switch (moduleId) {
        case 'aegis.domain.creator':
            return {
                tagline: 'Estudio completo para creadores de contenido en TikTok, Reels, YouTube, Shorts, Instagram y X.',
                highlights: [
                    'Banco de ideas y hooks virales por plataforma',
                    'Editor de guiones + Teleprompter en vivo integrado',
                    'Calendario editorial y puente directo hacia Video Studio IA',
                ],
            };
        case 'aegis.domain.video':
            return {
                tagline: 'Fábrica de Videos con IA: convierte ideas o guiones de Creator Studio en MP4 listos con subtítulos y voz.',
                highlights: [
                    'Selector universal: Gratis (Edge-TTS + Placas), Veo/Imagen, OpenAI, ElevenLabs o IA Local (ComfyUI)',
                    'Auto-instalación bajo demanda de FFmpeg y motor de voces en entorno aislado',
                    'Storyboard editable escena por escena con exportación real a MP4 + .SRT',
                ],
            };
        case 'aegis.domain.dev':
            return {
                tagline: 'Centro de ingeniería integrado con tus proyectos, Kanban técnico, snippets y agentes autónomos.',
                highlights: [
                    'Kanban de bugs, features, refactors y tareas técnicas',
                    'Registro de repositorios con comandos build/test en 1 clic',
                    'Bóveda de snippets y delegación directa a Agentes / Antigravity',
                ],
            };
        case 'aegis.domain.business':
            return {
                tagline: 'Mini-App de gestión comercial con catálogo, alertas de stock y Punto de Venta (POS) por voz o chat.',
                highlights: [
                    'Control de inventario con SKU/código de barras y alertas',
                    'Punto de Venta (POS) con descuento automático de stock',
                    'Actualizá precios o registrá ventas hablando con Aegis',
                ],
            };
        default:
            return {
                tagline: 'Módulo de dominio extensible del microkernel de Aegis.',
                highlights: ['Herramientas locales cifradas integradas al chat'],
            };
    }
}

export interface CreatorPostForm {
    title: string;
    platform: string;
    publish_at: string;
    caption: string;
    hashtags: string;
}

export type RunModuleAction = (
    moduleId: string,
    toolName: string,
    args: Record<string, unknown>,
) => Promise<void>;
