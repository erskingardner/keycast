<script lang="ts">
    import { tick } from "svelte";
    import { CRYPTO_METHODS, POLICY_USE_CASES, type CryptoMethod } from "$lib/policy/catalog";
    import { canAddKind, createEditorState, cryptoScope, cryptoSuggestions, defaultKinds, disableKindsEverywhere, kindName, kindSources, searchUseCases, selectUseCase, setKinds, signingKinds, type EditorState } from "$lib/policy/editor";
    import type { RecipientScope } from "$lib/types";

    let { value = $bindable(createEditorState()) }: { value: EditorState } = $props();
    const uid = $props.id();
    let query = $state("");
    let pickerOpen = $state(false);
    let picker: HTMLDivElement;
    let search: HTMLInputElement;
    let customKind = $state("");
    let customError = $state("");
    let legacyOpen = $state(false);
    const matches = $derived(searchUseCases(query));
    const categories = $derived([...new Set(matches.map(c => c.category))]);
    const selected = $derived(POLICY_USE_CASES.filter(c => c.id in value.selected));
    const kinds = $derived(signingKinds(value));
    const suggestions = $derived(cryptoSuggestions(value));
    const missing = $derived(CRYPTO_METHODS.filter(method => suggestions[method] && (cryptoScope(value, method) === "off" || (suggestions[method]?.scope === "any" && cryptoScope(value, method) !== "any"))));
    const activeCrypto = $derived(CRYPTO_METHODS.filter(m => cryptoScope(value, m) !== "off"));
    const hasLegacy = $derived(activeCrypto.some(m => m.startsWith("nip04")));

    function cryptoLabel(method: CryptoMethod) {
        return `${method.endsWith("encrypt") ? "Encrypt" : "Decrypt"} · ${method.startsWith("nip44") ? "NIP-44" : "NIP-04"}`;
    }
    function setCrypto(method: CryptoMethod, scope: RecipientScope | "off") {
        value = { ...value, overrides: { ...value.overrides, [method]: scope } };
    }
    function restoreSuggestions() {
        const overrides = { ...value.overrides };
        for (const method of missing) delete overrides[method];
        value = { ...value, overrides };
    }
    function closePicker() {
        search.focus();
        pickerOpen = false;
    }
    function addKind() {
        const raw = customKind.trim();
        const kind = Number(raw);
        if (!/^\d+$/.test(raw) || kind > 65535) {
            customError = "Enter a whole number from 0 to 65535.";
            return;
        }
        if (!canAddKind(kind)) {
            customError = "Keycast management approvals cannot be delegated.";
            return;
        }
        value = setKinds(value, "custom", [kind], true);
        customKind = "";
        customError = "";
    }
</script>

<svelte:window
    onpointerdown={(event) => { if (picker && event.target instanceof Node && !picker.contains(event.target)) pickerOpen = false; }}
    onkeydown={(event) => { if (event.key === "Escape" && pickerOpen && picker?.contains(document.activeElement)) { event.preventDefault(); event.stopPropagation(); closePicker(); } }}
/>

<div class="policy-permissions">
    <div class="section-heading">
        <h3><label for={`${uid}-search`}>Start with a use case</label></h3>
        <span class="text-sm text-muted">Choose one or more</span>
    </div>
    <div class="use-case-picker" bind:this={picker} role="group" aria-label="Choose use cases"
        onfocusout={() => { setTimeout(() => { if (!picker?.contains(document.activeElement)) pickerOpen = false; }, 0); }}>
        <input id={`${uid}-search`} type="search" role="combobox" aria-haspopup="dialog" placeholder="Search use cases, NIPs, or event kinds…"
            bind:this={search} bind:value={query} autocomplete="off" aria-expanded={pickerOpen}
            aria-controls={`${uid}-options`} onfocus={() => pickerOpen = true} onclick={() => pickerOpen = true}
            oninput={() => pickerOpen = true}
            onkeydown={async (event) => {
                if (event.key === "Enter") event.preventDefault();
                if (event.key === "ArrowDown") {
                    event.preventDefault(); pickerOpen = true;
                    await tick();
                    picker.querySelector<HTMLInputElement>("[data-use-case]")?.focus();
                }
            }} />
        <div id={`${uid}-options`} class="use-case-menu" role="dialog" aria-label="Use case options" hidden={!pickerOpen}>
            <div class="menu-heading"><span class="text-sm text-muted" role="status">{matches.length} use cases</span>
                <button type="button" class="button button-secondary" onclick={closePicker}>Done</button></div>
            <div class="use-case-options" role="group" aria-label="Available use cases">
                {#each categories as category (category)}
                    <div class="category-label">{category}</div>
                    {#each matches.filter(c => c.category === category) as useCase (useCase.id)}
                        <label class="option-row">
                            <input type="checkbox" data-use-case={useCase.id} checked={useCase.id in value.selected}
                                onchange={(event) => value = selectUseCase(value, useCase, event.currentTarget.checked)} />
                            <span class="option-copy"><span class="option-title">{useCase.name}</span><span>{useCase.description}</span></span>
                        </label>
                    {/each}
                {/each}
                {#if !matches.length}<p class="text-sm text-muted py-4">No matching use cases. Try a different search or add an individual kind below.</p>{/if}
            </div>
        </div>
    </div>

    <div class="section-heading"><h3>Selected use cases</h3><span class="text-sm text-muted" role="status">{selected.length} selected</span></div>
    {#each selected as useCase (useCase.id)}
        {@const enabled = value.selected[useCase.id]}
        {@const defaults = defaultKinds(useCase)}
        {@const customized = defaults.length !== enabled.length || defaults.some(k => !enabled.includes(k))}
        <details class="permission-group" data-policy-group={useCase.id}>
            <summary>{useCase.name} <span class="text-muted">· {enabled.length} {enabled.length === 1 ? "kind" : "kinds"}{customized ? " · Customized" : ""}</span></summary>
            <div class="group-body">
                <div class="group-heading"><span class="text-sm text-muted">{useCase.description}</span>
                    <button type="button" class="button button-secondary" aria-label={`Remove ${useCase.name}`}
                        onclick={() => value = selectUseCase(value, useCase, false)}>Remove</button></div>
                {#each useCase.items as item (item.name)}
                    {@const checked = item.kinds.every(k => enabled.includes(k))}
                    {@const shared = [...new Set(item.kinds.flatMap(k => kindSources(value, k, useCase.id)))]}
                    <label class="capability-row">
                        <input type="checkbox" {checked} indeterminate={!checked && item.kinds.some(k => enabled.includes(k))}
                            onchange={(event) => value = setKinds(value, useCase.id, item.kinds, event.currentTarget.checked)} />
                        <span class="capability-name">{item.name}</span><span class="kind-number">{item.kinds.join(", ")}</span>
                    </label>
                    {#if shared.length}<div class="shared-note">
                        {checked ? "Also included in" : "Still enabled by"} {shared.join(", ")}
                        {#if !checked}<button type="button" class="button button-secondary" onclick={() => value = disableKindsEverywhere(value, item.kinds)}>Disable everywhere</button>{/if}
                    </div>{/if}
                {/each}
                {#if useCase.transport?.length}
                    <div class="transport-notes"><h4>Handled by the app</h4>
                        {#each useCase.transport as transport (transport.kind)}<p><span class="text-ink">{transport.name} · {transport.kind}</span><br />{transport.description}</p>{/each}
                    </div>
                {/if}
                {#each useCase.notes ?? [] as note}<p class="text-sm text-muted mt-3">{note}</p>{/each}
            </div>
        </details>
    {/each}
    {#if !selected.length}<p class="text-sm text-muted">Choose a use case above, or edit individual permissions below.</p>{/if}

    {#if value.customKinds.length}
        <details class="permission-group" open data-policy-group="custom">
            <summary>Individual permissions <span class="text-muted">· {value.customKinds.length} kinds</span></summary>
            <div class="group-body">
                <p class="text-sm text-muted mb-2">Saved policies retain their exact permissions. Adding a use case adds its selected capabilities.</p>
                {#each [...value.customKinds].sort((a,b) => a-b) as kind (kind)}
                    <div class="individual-row"><span>{kindName(kind)} <span class="kind-number">{kind}</span></span>
                        <button type="button" class="button button-secondary" aria-label={`Remove kind ${kind}`} onclick={() => value = setKinds(value, "custom", [kind], false)}>Remove</button></div>
                    {#if kindSources(value, kind, "custom").length}<p class="shared-note">Also included in {kindSources(value, kind, "custom").join(", ")}</p>{/if}
                {/each}
            </div>
        </details>
    {/if}

    <div class="section-heading"><h3>Encryption and decryption</h3><span class="text-sm text-muted">Separate permissions</span></div>
    {#snippet cryptoControl(method: CryptoMethod)}
        {@const scope = cryptoScope(value, method)}
        <div class="crypto-row">
            <label class="crypto-label"><input type="checkbox" checked={scope !== "off"}
                onchange={(event) => setCrypto(method, event.currentTarget.checked ? suggestions[method]?.scope ?? value.savedCrypto[method] ?? "self_only" : "off")} />
                <span class="option-copy"><span class="option-title">{cryptoLabel(method)}</span><span>
                    {suggestions[method] ? `Suggested for ${suggestions[method]!.sources.join(", ")}` : value.savedCrypto[method] ? "Saved permission" : "Optional"}
                </span></span>
            </label>
            <select aria-label={`${cryptoLabel(method)} scope`} value={scope === "off" ? "self_only" : scope} disabled={scope === "off"}
                onchange={(event) => setCrypto(method, event.currentTarget.value as RecipientScope)}>
                <option value="self_only">This key only</option><option value="any">Any public key</option>
            </select>
        </div>
    {/snippet}
    {@render cryptoControl("nip44_encrypt")}
    {@render cryptoControl("nip44_decrypt")}
    <details class="legacy-crypto" open={legacyOpen || hasLegacy} ontoggle={(event) => legacyOpen = event.currentTarget.open}>
        <summary>Legacy encryption · NIP-04</summary>
        {@render cryptoControl("nip04_encrypt")}
        {@render cryptoControl("nip04_decrypt")}
    </details>
    {#if activeCrypto.length}<p class="text-sm text-muted mt-3">“This key only” covers data encrypted to this identity, including data outside the selected use cases.</p>{/if}
    {#if missing.length}<div class="crypto-warning" role="status"><p>Some selected features need broader encryption access: {missing.map(cryptoLabel).join(", ")}.</p>
        <button type="button" class="button button-secondary" onclick={restoreSuggestions}>Use suggested access</button></div>{/if}

    <details class="custom-kind"><summary>Add an unlisted event kind</summary>
        <div class="custom-kind-input"><label for={`${uid}-kind`} class="sr-only">Event kind number</label>
            <input id={`${uid}-kind`} type="text" inputmode="numeric" placeholder="Event kind, 0–65535" bind:value={customKind}
                onkeydown={(event) => { if (event.key === "Enter") { event.preventDefault(); addKind(); } }} />
            <button type="button" class="button button-secondary" onclick={addKind}>Add kind</button></div>
        {#if customError}<p class="input-error mt-2" role="alert">{customError}</p>{/if}
    </details>

    <details class="effective-access"><summary>Effective access · {kinds.length} signing kinds · {activeCrypto.length} cryptographic permissions</summary>
        <p class="text-sm text-muted my-3">Permissions combine across use cases. No other access is allowed.</p>
        {#if kinds.length}<div class="effective-kinds">{#each kinds as kind (kind)}<div>{kindName(kind)} <span class="kind-number">{kind}</span></div>{/each}</div>
        {:else}<p class="text-sm text-muted">No event signing allowed.</p>{/if}
        {#each activeCrypto as method}<p class="text-sm mt-2">{cryptoLabel(method)} · {cryptoScope(value, method) === "any" ? "Any public key" : "This key only"}</p>{/each}
    </details>
</div>

<style>
    .policy-permissions { min-width: 0; }
    .section-heading { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; margin: 22px 0 10px; }
    .section-heading:first-child { margin-top: 0; }
    h3 { font-size: 14px; font-weight: 600; }
    .use-case-picker { position: relative; }
    .use-case-menu { position: absolute; top: calc(100% + 6px); left: 0; right: 0; z-index: 20; background: var(--color-surface); border: 1px solid var(--color-line); box-shadow: 0 8px 24px #0006; }
    .use-case-menu[hidden] { display: none; }
    .menu-heading { display: flex; justify-content: space-between; align-items: center; gap: 10px; padding: 10px 14px; border-bottom: 1px solid var(--color-line); }
    .use-case-options { max-height: 320px; overflow-y: auto; overscroll-behavior: contain; padding: 0 14px 10px; }
    .category-label { padding: 14px 0 5px; font-size: 12px; font-weight: 600; color: var(--color-muted); }
    .option-row, .capability-row { display: flex; align-items: center; gap: 12px; padding: 12px 0; cursor: pointer; }
    .option-row { border-bottom: 1px solid var(--color-line); }
    .option-row:hover { background: var(--color-canvas); }
    input[type="checkbox"] { flex-shrink: 0; }
    .option-copy { display: grid; gap: 3px; min-width: 0; }
    .option-title, .capability-name { color: var(--color-ink); font-size: 14px; }
    .permission-group { margin-top: 10px; border: 1px solid var(--color-line); background: var(--color-surface); }
    .permission-group > summary { padding: 14px; font-size: 14px; }
    .group-body { padding: 0 14px 14px; }
    .group-heading, .individual-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 8px 0; }
    .group-heading { border-bottom: 1px solid var(--color-line); padding-bottom: 12px; }
    .capability-name { flex: 1; }
    .kind-number { font-family: var(--font-mono); font-size: 12px; color: var(--color-muted); overflow-wrap: anywhere; }
    .shared-note { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin: 0 0 8px 28px; font-size: 12px; color: var(--color-muted); }
    .transport-notes { border-top: 1px solid var(--color-line); padding-top: 12px; margin-top: 8px; font-size: 12px; color: var(--color-muted); }
    .transport-notes h4 { color: var(--color-ink); font-weight: 600; }
    .transport-notes p { margin-top: 10px; }
    .crypto-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 0; border-bottom: 1px solid var(--color-line); }
    .crypto-label { display: flex; align-items: center; gap: 12px; cursor: pointer; flex: 1; }
    .crypto-row select { width: auto; max-width: 100%; }
    .crypto-warning { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; color: var(--color-warning); font-size: 13px; margin-top: 12px; }
    .legacy-crypto, .custom-kind { margin-top: 16px; font-size: 13px; }
    .legacy-crypto > summary, .custom-kind > summary { padding: 8px 0; }
    .custom-kind-input { display: flex; gap: 8px; margin-top: 10px; }
    .custom-kind-input button { flex-shrink: 0; }
    .effective-access { margin-top: 20px; padding-top: 16px; border-top: 1px solid var(--color-line); font-size: 13px; }
    .effective-kinds { display: grid; gap: 6px; font-size: 13px; }
    @media (max-width: 520px) {
        input[type="search"], input[type="text"], .crypto-row select { font-size: 16px; }
        .crypto-row { flex-wrap: wrap; }
        .crypto-row select { width: 100%; }
        .crypto-label { flex-basis: 100%; }
        .capability-row { flex-wrap: wrap; }
        .group-heading { align-items: flex-start; }
    }
</style>
