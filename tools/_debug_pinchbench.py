import requests, re, json

headers = {'User-Agent': 'Mozilla/5.0 Chrome/124.0'}
resp = requests.get('https://pinchbench.com/', headers=headers, timeout=20)
html = resp.text

# Extraer todos los chunks de next_f
chunks = re.findall(r'self\.__next_f\.push\(\[1,"(.*?)"\]\)', html, re.DOTALL)
print(f"Total chunks: {len(chunks)}")

# Buscar el chunk que contiene model IDs Y porcentajes
for i, chunk in enumerate(chunks):
    if 'anthropic/claude' in chunk and '%' in chunk:
        print(f"\n=== CHUNK {i} tiene models + % ===")
        print(chunk[:3000])
        break

# Buscar porcentajes cerca de model IDs en todo el HTML
print("\n\n=== BUSQUEDA DIRECTA EN HTML ===")
# Buscar patron: model_id seguido de score en los proximos 200 chars
for m in re.finditer(r'((?:anthropic|openai|google|meta-llama|mistral|groq|deepseek|qwen|x-ai)[/][a-z0-9\-\.]+)', html):
    snippet = html[m.start():m.start()+200]
    pcts = re.findall(r'(\d+\.\d+)', snippet)
    if pcts:
        print(f"MODEL: {m.group(1)}")
        print(f"  SNIPPET: {snippet[:150]}")
        print(f"  NUMBERS: {pcts[:5]}")
        print()
