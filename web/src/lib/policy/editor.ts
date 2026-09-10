import type { PolicyDocument, RecipientScope } from "$lib/types";
import { CRYPTO_METHODS, POLICY_USE_CASES, type CryptoMethod, type CryptoPermissions, type PolicyUseCase } from "./catalog";

export type EditorState = {
    selected: Record<string, number[]>;
    customKinds: number[];
    savedCrypto: CryptoPermissions;
    overrides: Partial<Record<CryptoMethod, RecipientScope | "off">>;
};

const blockedKinds = new Set([27236, 27237]);
export const isKind = (kind: unknown): kind is number => Number.isInteger(kind) && Number(kind) >= 0 && Number(kind) <= 65535;
export const canAddKind = (kind: number) => isKind(kind) && !blockedKinds.has(kind);
export const defaultKinds = (useCase: PolicyUseCase) => [...new Set(useCase.items.filter(item => item.default).flatMap(item => item.kinds))];

// API documents are runtime input. Refuse unknown or incomplete data instead of
// silently dropping a capability when editing a policy from a newer server.
export function validateDocument(value: unknown): asserts value is PolicyDocument {
    const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v);
    const fail = () => { throw new Error("This policy has an unsupported or invalid document and cannot be edited safely."); };
    if (!object(value) || value.version !== 1 || Object.keys(value).some(k => !["version", "capabilities"].includes(k))) return fail();
    const caps = value.capabilities;
    if (!object(caps) || !Object.keys(caps).length) return fail();
    for (const [method, capability] of Object.entries(caps)) {
        if (!object(capability)) return fail();
        if (method === "sign_event") {
            const kinds = capability.allowed_kinds;
            if (Object.keys(capability).length !== 1 || !Array.isArray(kinds) || !kinds.length || kinds.some(k => !isKind(k)) || new Set(kinds).size !== kinds.length) return fail();
        } else if (CRYPTO_METHODS.includes(method as CryptoMethod)) {
            if (Object.keys(capability).length !== 1 || (capability.recipient !== "any" && capability.recipient !== "self_only")) return fail();
        } else return fail();
    }
}

export function createEditorState(document?: PolicyDocument): EditorState {
    if (document) validateDocument(document);
    const savedCrypto: CryptoPermissions = {};
    for (const method of CRYPTO_METHODS) {
        const capability = document?.capabilities[method];
        if (capability) savedCrypto[method] = capability.recipient;
    }
    // Template provenance is not part of the strict policy contract. Preserve exact
    // kinds as editable individual permissions; never guess templates on reload.
    return { selected: {}, customKinds: [...(document?.capabilities.sign_event?.allowed_kinds ?? [])], savedCrypto, overrides: {} };
}

export function signingKinds(state: EditorState): number[] {
    return [...new Set([...state.customKinds, ...Object.values(state.selected).flat()])].sort((a, b) => a - b);
}

export function selectUseCase(state: EditorState, useCase: PolicyUseCase, checked: boolean): EditorState {
    const selected = { ...state.selected };
    if (checked) {
        // Checking an already selected search result must not reset customization.
        if (!(useCase.id in selected)) selected[useCase.id] = defaultKinds(useCase);
    } else delete selected[useCase.id];
    return { ...state, selected };
}

export function setKinds(state: EditorState, group: string, kinds: number[], checked: boolean): EditorState {
    const values = group === "custom" ? state.customKinds : state.selected[group] ?? [];
    const next = checked ? [...new Set([...values, ...kinds])] : values.filter(k => !kinds.includes(k));
    return group === "custom" ? { ...state, customKinds: next } : { ...state, selected: { ...state.selected, [group]: next } };
}

export function disableKindsEverywhere(state: EditorState, kinds: number[]): EditorState {
    return { ...state, customKinds: state.customKinds.filter(k => !kinds.includes(k)), selected: Object.fromEntries(Object.entries(state.selected).map(([id, values]) => [id, values.filter(k => !kinds.includes(k))])) };
}

export function kindSources(state: EditorState, kind: number, excluding?: string): string[] {
    const sources = POLICY_USE_CASES.filter(c => c.id !== excluding && state.selected[c.id]?.includes(kind)).map(c => c.name);
    if (excluding !== "custom" && state.customKinds.includes(kind)) sources.push("Individual permissions");
    return sources;
}

export function cryptoSuggestions(state: EditorState): Partial<Record<CryptoMethod, { scope: RecipientScope; sources: string[] }>> {
    const result: ReturnType<typeof cryptoSuggestions> = {};
    function add(permissions: CryptoPermissions, source: string) {
        for (const method of CRYPTO_METHODS) {
            const scope = permissions[method];
            if (!scope) continue;
            const previous = result[method];
            result[method] = { scope: previous?.scope === "any" || scope === "any" ? "any" : "self_only", sources: [...new Set([...(previous?.sources ?? []), source])] };
        }
    }
    for (const c of POLICY_USE_CASES) {
        const kinds = state.selected[c.id];
        if (!kinds) continue;
        add(c.crypto ?? {}, c.name);
        for (const item of c.items) if (item.kinds.some(k => kinds.includes(k))) add(item.crypto ?? {}, c.name);
    }
    return result;
}

export function cryptoScope(state: EditorState, method: CryptoMethod): RecipientScope | "off" {
    if (state.overrides[method] !== undefined) return state.overrides[method];
    const saved = state.savedCrypto[method];
    const suggested = cryptoSuggestions(state)[method]?.scope;
    return saved === "any" || suggested === "any" ? "any" : saved ?? suggested ?? "off";
}

export function buildDocument(state: EditorState): PolicyDocument {
    const capabilities: PolicyDocument["capabilities"] = {};
    const kinds = signingKinds(state);
    if (kinds.length) capabilities.sign_event = { allowed_kinds: kinds };
    for (const method of CRYPTO_METHODS) {
        const recipient = cryptoScope(state, method);
        if (recipient !== "off") capabilities[method] = { recipient };
    }
    if (!Object.keys(capabilities).length) throw new Error("Choose at least one signing, encryption, or decryption permission.");
    const document: PolicyDocument = { version: 1, capabilities };
    validateDocument(document);
    return document;
}

export function searchUseCases(query: string): readonly PolicyUseCase[] {
    const tokens = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
    return POLICY_USE_CASES.filter(c => {
        const text = [c.name, c.description, c.category, ...c.nips.flatMap(n => [n, `nip-${n}`, `nip ${n}`]), ...c.items.map(i => `${i.name} ${i.kinds.join(" ")}`), ...(c.transport ?? []).map(i => `${i.name} ${i.kind}`)].join(" ").toLowerCase();
        return tokens.every(t => text.includes(t));
    });
}

export function kindName(kind: number): string {
    return POLICY_USE_CASES.flatMap(c => c.items).find(i => i.kinds.includes(kind))?.name ?? (blockedKinds.has(kind) ? "Private management (always denied)" : `Event kind ${kind}`);
}
