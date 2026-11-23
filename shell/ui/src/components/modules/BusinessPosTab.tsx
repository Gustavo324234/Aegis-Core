import React, { useState } from 'react';
import {
    Trash2,
    ShoppingBag,
    Package,
    Search,
} from 'lucide-react';
import { BizOverview, RunModuleAction } from './types';

export interface BusinessPosTabProps {
    bizData: BizOverview;
    busyTool: boolean;
    runActionAndRefresh: RunModuleAction;
}

export const BusinessPosTab: React.FC<BusinessPosTabProps> = ({
    bizData,
    busyTool,
    runActionAndRefresh,
}) => {
    const [bizTab, setBizTab] = useState<'pos' | 'inventory'>('pos');
    const [posProductKey, setPosProductKey] = useState('');
    const [posQty, setPosQty] = useState(1);
    const [posUnitPrice, setPosUnitPrice] = useState<string>('');
    const [posCustomer, setPosCustomer] = useState('Consumidor Final');
    const [posPayment, setPosPayment] = useState('efectivo');

    const [newProductForm, setNewProductForm] = useState({
        barcode: '',
        name: '',
        category: 'General',
        price: '',
        cost: '',
        stock: '1',
        min_stock: '3',
    });
    const [productSearch, setProductSearch] = useState('');

    const selectedPosProduct = bizData.products.find((p) => p.id === posProductKey) ?? null;
    const effectivePosPrice =
        posUnitPrice.trim() !== ''
            ? Number(posUnitPrice) || 0
            : selectedPosProduct?.price ?? 0;

    return (
        <div className="space-y-5">
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Productos</p>
                    <p className="text-2xl font-bold text-white mt-1">{bizData.stats.total_products}</p>
                </div>
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Alertas Stock Bajo</p>
                    <p
                        className={`text-2xl font-bold mt-1 ${
                            bizData.stats.low_stock_count > 0 ? 'text-amber-300' : 'text-emerald-300'
                        }`}
                    >
                        {bizData.stats.low_stock_count}
                    </p>
                </div>
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Valor Inventario</p>
                    <p className="text-2xl font-bold text-aegis-cyan mt-1">
                        ${bizData.stats.inventory_value.toLocaleString('es-AR')}
                    </p>
                </div>
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Ingresos Ventas</p>
                    <p className="text-2xl font-bold text-emerald-300 mt-1">
                        ${bizData.stats.total_revenue.toLocaleString('es-AR')}
                    </p>
                </div>
            </div>

            <div className="flex items-center gap-2 border-b border-white/10 pb-3">
                <button
                    onClick={() => setBizTab('pos')}
                    className={`px-3.5 py-2 rounded-xl text-xs font-mono flex items-center gap-2 border transition-all ${
                        bizTab === 'pos'
                            ? 'bg-emerald-500/20 text-emerald-200 border-emerald-400/40'
                            : 'bg-white/5 text-white/55 border-white/10 hover:text-white'
                    }`}
                >
                    <ShoppingBag className="w-3.5 h-3.5" />
                    Punto de Venta (POS)
                </button>
                <button
                    onClick={() => setBizTab('inventory')}
                    className={`px-3.5 py-2 rounded-xl text-xs font-mono flex items-center gap-2 border transition-all ${
                        bizTab === 'inventory'
                            ? 'bg-emerald-500/20 text-emerald-200 border-emerald-400/40'
                            : 'bg-white/5 text-white/55 border-white/10 hover:text-white'
                    }`}
                >
                    <Package className="w-3.5 h-3.5" />
                    Catálogo & Stock
                </button>
            </div>

            {/* Biz Tab 1: POS */}
            {bizTab === 'pos' && (
                <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                    <div className="lg:col-span-5 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-3 h-fit">
                        <h4 className="text-xs font-mono uppercase text-emerald-300 font-bold">
                            Nueva Venta (descuenta stock automático)
                        </h4>
                        <select
                            value={posProductKey}
                            onChange={(e) => {
                                setPosProductKey(e.target.value);
                                const found = bizData.products.find((p) => p.id === e.target.value);
                                if (found) setPosUnitPrice(String(found.price));
                            }}
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        >
                            <option value="">-- Elegí un producto del catálogo --</option>
                            {bizData.products.map((p) => (
                                <option key={p.id} value={p.id}>
                                    {p.name} (Stock: {p.stock} · ${p.price})
                                </option>
                            ))}
                        </select>
                        <div className="grid grid-cols-2 gap-2">
                            <div>
                                <label className="text-[10px] font-mono text-white/40 block mb-1">
                                    Cantidad
                                </label>
                                <input
                                    type="number"
                                    min={1}
                                    value={posQty}
                                    onChange={(e) => setPosQty(Math.max(1, Number(e.target.value) || 1))}
                                    className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                />
                            </div>
                            <div>
                                <label className="text-[10px] font-mono text-white/40 block mb-1">
                                    Precio Unitario ($)
                                </label>
                                <input
                                    type="number"
                                    value={posUnitPrice}
                                    onChange={(e) => setPosUnitPrice(e.target.value)}
                                    placeholder="Precio"
                                    className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                />
                            </div>
                        </div>
                        <div className="grid grid-cols-2 gap-2">
                            <input
                                value={posCustomer}
                                onChange={(e) => setPosCustomer(e.target.value)}
                                placeholder="Cliente"
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                            <select
                                value={posPayment}
                                onChange={(e) => setPosPayment(e.target.value)}
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            >
                                <option value="efectivo">Efectivo</option>
                                <option value="transferencia">Transferencia</option>
                                <option value="tarjeta">Tarjeta</option>
                                <option value="qr">QR</option>
                            </select>
                        </div>
                        <div className="p-3 rounded-xl bg-emerald-500/10 border border-emerald-400/20 flex items-center justify-between text-sm font-mono">
                            <span className="text-white/70">TOTAL:</span>
                            <span className="font-bold text-emerald-300">
                                ${(effectivePosPrice * posQty).toLocaleString('es-AR')}
                            </span>
                        </div>
                        <button
                            disabled={busyTool || !posProductKey}
                            onClick={async () => {
                                await runActionAndRefresh('aegis.domain.business', 'biz_record_sale', {
                                    product_id: posProductKey,
                                    quantity: posQty,
                                    unit_price: effectivePosPrice,
                                    customer: posCustomer.trim() || 'Consumidor Final',
                                    payment_method: posPayment,
                                });
                                setPosQty(1);
                            }}
                            className="w-full py-2.5 rounded-xl bg-emerald-500 hover:bg-emerald-400 disabled:opacity-40 text-black font-mono text-xs font-bold"
                        >
                            Cobrar y Registrar Venta
                        </button>
                    </div>

                    <div className="lg:col-span-7 space-y-2.5">
                        <h4 className="text-xs font-mono uppercase text-white/50">
                            Últimas Ventas ({bizData.sales.length})
                        </h4>
                        {bizData.sales.length === 0 ? (
                            <div className="p-8 rounded-2xl border border-dashed border-white/15 text-center text-xs font-mono text-white/40">
                                Sin ventas registradas aún. También podés decirle a Aegis: "Vendí 2 unidades de X".
                            </div>
                        ) : (
                            <div className="space-y-2">
                                {bizData.sales.slice(0, 20).map((s) => (
                                    <div
                                        key={s.id}
                                        className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10 flex items-center justify-between gap-3"
                                    >
                                        <div>
                                            <p className="text-xs font-semibold text-white">
                                                {s.quantity}× {s.product_name}
                                            </p>
                                            <p className="text-[10px] font-mono text-white/45">
                                                {s.customer} · {s.payment_method} · {s.created_at}
                                            </p>
                                        </div>
                                        <span className="text-sm font-mono font-bold text-emerald-300">
                                            +${s.total.toLocaleString('es-AR')}
                                        </span>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </div>
            )}

            {/* Biz Tab 2: Inventory */}
            {bizTab === 'inventory' && (
                <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                    <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-2.5 h-fit">
                        <h4 className="text-xs font-mono uppercase text-emerald-300 font-bold">
                            + Agregar Producto
                        </h4>
                        <input
                            value={newProductForm.name}
                            onChange={(e) => setNewProductForm({ ...newProductForm, name: e.target.value })}
                            placeholder="Nombre del producto"
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <div className="grid grid-cols-2 gap-2">
                            <input
                                value={newProductForm.barcode}
                                onChange={(e) => setNewProductForm({ ...newProductForm, barcode: e.target.value })}
                                placeholder="SKU / Código"
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                            <input
                                value={newProductForm.category}
                                onChange={(e) => setNewProductForm({ ...newProductForm, category: e.target.value })}
                                placeholder="Categoría"
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                        </div>
                        <div className="grid grid-cols-2 gap-2">
                            <input
                                type="number"
                                value={newProductForm.price}
                                onChange={(e) => setNewProductForm({ ...newProductForm, price: e.target.value })}
                                placeholder="Precio venta ($)"
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                            <input
                                type="number"
                                value={newProductForm.cost}
                                onChange={(e) => setNewProductForm({ ...newProductForm, cost: e.target.value })}
                                placeholder="Costo ($)"
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                        </div>
                        <div className="grid grid-cols-2 gap-2">
                            <input
                                type="number"
                                value={newProductForm.stock}
                                onChange={(e) => setNewProductForm({ ...newProductForm, stock: e.target.value })}
                                placeholder="Stock"
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                            <input
                                type="number"
                                value={newProductForm.min_stock}
                                onChange={(e) => setNewProductForm({ ...newProductForm, min_stock: e.target.value })}
                                placeholder="Stock mínimo"
                                className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                        </div>
                        <button
                            disabled={busyTool || !newProductForm.name.trim() || !newProductForm.price}
                            onClick={async () => {
                                await runActionAndRefresh('aegis.domain.business', 'biz_add_product', {
                                    barcode: newProductForm.barcode.trim(),
                                    name: newProductForm.name.trim(),
                                    category: newProductForm.category.trim() || 'General',
                                    price: Number(newProductForm.price) || 0,
                                    cost: Number(newProductForm.cost) || 0,
                                    stock: Number(newProductForm.stock) || 0,
                                    min_stock: Number(newProductForm.min_stock) || 3,
                                });
                                setNewProductForm({
                                    barcode: '',
                                    name: '',
                                    category: 'General',
                                    price: '',
                                    cost: '',
                                    stock: '1',
                                    min_stock: '3',
                                });
                            }}
                            className="w-full py-2 rounded-xl bg-emerald-500 hover:bg-emerald-400 disabled:opacity-40 text-black font-mono text-xs font-bold"
                        >
                            Guardar Producto
                        </button>
                    </div>

                    <div className="lg:col-span-8 space-y-3">
                        <div className="relative">
                            <Search className="w-3.5 h-3.5 text-white/35 absolute left-3 top-2.5" />
                            <input
                                value={productSearch}
                                onChange={(e) => setProductSearch(e.target.value)}
                                placeholder="Buscar por nombre, categoría o SKU..."
                                className="w-full pl-9 pr-3 py-2 rounded-lg bg-white/5 border border-white/10 text-xs text-white outline-none"
                            />
                        </div>
                        <div className="space-y-2">
                            {bizData.products
                                .filter(
                                    (p) =>
                                        !productSearch.trim() ||
                                        `${p.name} ${p.category} ${p.barcode}`
                                            .toLowerCase()
                                            .includes(productSearch.toLowerCase()),
                                )
                                .map((prod) => {
                                    const low = prod.stock <= prod.min_stock;
                                    return (
                                        <div
                                            key={prod.id}
                                            className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10 flex flex-wrap items-center justify-between gap-3"
                                        >
                                            <div className="min-w-0">
                                                <div className="flex items-center gap-2 flex-wrap">
                                                    <h5 className="text-sm font-semibold text-white">{prod.name}</h5>
                                                    <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-white/5 text-white/55">
                                                        {prod.category}
                                                    </span>
                                                    {low && (
                                                        <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/20 text-amber-300 border border-amber-400/30">
                                                            ⚠ Stock Bajo
                                                        </span>
                                                    )}
                                                </div>
                                                <p className="text-[11px] font-mono text-white/45 mt-0.5">
                                                    Venta: ${prod.price.toLocaleString('es-AR')}
                                                    {prod.barcode ? ` · SKU: ${prod.barcode}` : ''}
                                                </p>
                                            </div>
                                            <div className="flex items-center gap-2">
                                                <button
                                                    onClick={() =>
                                                        void runActionAndRefresh(
                                                            'aegis.domain.business',
                                                            'biz_update_stock',
                                                            { product_id: prod.id, quantity_change: -1 },
                                                        )
                                                    }
                                                    className="w-7 h-7 rounded-lg bg-white/5 hover:bg-white/15 text-white font-mono text-xs"
                                                >
                                                    -1
                                                </button>
                                                <span
                                                    className={`min-w-[44px] text-center font-mono text-sm font-bold ${
                                                        low ? 'text-amber-300' : 'text-emerald-300'
                                                    }`}
                                                >
                                                    {prod.stock}
                                                </span>
                                                <button
                                                    onClick={() =>
                                                        void runActionAndRefresh(
                                                            'aegis.domain.business',
                                                            'biz_update_stock',
                                                            { product_id: prod.id, quantity_change: 1 },
                                                        )
                                                    }
                                                    className="w-7 h-7 rounded-lg bg-white/5 hover:bg-white/15 text-white font-mono text-xs"
                                                >
                                                    +1
                                                </button>
                                                <button
                                                    onClick={() =>
                                                        void runActionAndRefresh(
                                                            'aegis.domain.business',
                                                            'biz_delete_product',
                                                            { id: prod.id },
                                                        )
                                                    }
                                                    className="p-1.5 text-white/30 hover:text-red-400"
                                                >
                                                    <Trash2 className="w-3.5 h-3.5" />
                                                </button>
                                            </div>
                                        </div>
                                    );
                                })}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
