<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import type { Policy, PolicyDocument, RecipientScope, TeamWithRelations } from "$lib/types";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;
const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
const editPolicyId = Number($page.url.searchParams.get("edit"));
const isEditing = Number.isInteger(editPolicyId) && editPolicyId > 0;

let policyName = $state("");
let allowSigning = $state(true);
let allowedKinds = $state("1, 7");
let nip04Encrypt = $state(false);
let nip04Decrypt = $state(false);
let nip44Encrypt = $state(false);
let nip44Decrypt = $state(false);
let nip04EncryptRecipient: RecipientScope = $state("any");
let nip04DecryptRecipient: RecipientScope = $state("any");
let nip44EncryptRecipient: RecipientScope = $state("any");
let nip44DecryptRecipient: RecipientScope = $state("any");
let errorMessage: string | null = $state(null);
let isSaving = $state(false);
let editLoadStarted = $state(false);
let editLoaded = $state(!isEditing);

$effect(() => {
    if (!isEditing || editLoadStarted || !user?.pubkey) return;
    editLoadStarted = true;
    const endpoint = `/teams/${id}`;
    api.buildAuthHeader(endpoint, "GET", user.pubkey)
        .then((authorization) => api.get<TeamWithRelations>(endpoint, { headers: { Authorization: authorization } }))
        .then((team) => {
            const policy = team.policies.find((item) => item.id === editPolicyId);
            if (!policy) throw new Error("Policy not found");
            loadPolicy(policy);
            editLoaded = true;
        })
        .catch((error) => {
            errorMessage = error instanceof Error ? error.message : String(error);
            editLoaded = true;
        });
});

function loadPolicy(policy: Policy) {
    policyName = policy.name;
    const capabilities = policy.document.capabilities;
    allowSigning = !!capabilities.sign_event;
    allowedKinds = capabilities.sign_event?.allowed_kinds.join(", ") ?? "";
    nip04Encrypt = !!capabilities.nip04_encrypt;
    nip04Decrypt = !!capabilities.nip04_decrypt;
    nip44Encrypt = !!capabilities.nip44_encrypt;
    nip44Decrypt = !!capabilities.nip44_decrypt;
    nip04EncryptRecipient = capabilities.nip04_encrypt?.recipient ?? "any";
    nip04DecryptRecipient = capabilities.nip04_decrypt?.recipient ?? "any";
    nip44EncryptRecipient = capabilities.nip44_encrypt?.recipient ?? "any";
    nip44DecryptRecipient = capabilities.nip44_decrypt?.recipient ?? "any";
}

function parseKinds(): number[] | null {
    const values = allowedKinds.split(",").map((value) => value.trim()).filter(Boolean);
    const parsed = values.map(Number);
    if (parsed.length === 0 || parsed.some((value) => !Number.isInteger(value) || value < 0 || value > 65535)) {
        return null;
    }
    return [...new Set(parsed)];
}

async function savePolicy() {
    if (!user?.pubkey || isSaving) return;
    errorMessage = null;
    const kinds = allowSigning ? parseKinds() : [];
    if (allowSigning && !kinds) {
        errorMessage = "Signing kinds must be a non-empty comma-separated list from 0 to 65535.";
        return;
    }

    const capabilities: PolicyDocument["capabilities"] = {};
    if (allowSigning) capabilities.sign_event = { allowed_kinds: kinds! };
    if (nip04Encrypt) capabilities.nip04_encrypt = { recipient: nip04EncryptRecipient };
    if (nip04Decrypt) capabilities.nip04_decrypt = { recipient: nip04DecryptRecipient };
    if (nip44Encrypt) capabilities.nip44_encrypt = { recipient: nip44EncryptRecipient };
    if (nip44Decrypt) capabilities.nip44_decrypt = { recipient: nip44DecryptRecipient };
    if (Object.keys(capabilities).length === 0) {
        errorMessage = "Choose at least one capability. Empty policies are intentionally rejected.";
        return;
    }

    const request = { name: policyName, document: { version: 1, capabilities } as PolicyDocument };
    isSaving = true;
    try {
        const body = JSON.stringify(request);
        const endpoint = isEditing ? `/teams/${id}/policies/${editPolicyId}` : `/teams/${id}/policies`;
        const method = isEditing ? "PUT" : "POST";
        const authHeader = await api.buildAuthHeader(endpoint, method, user.pubkey, body);
        if (isEditing) {
            await api.put(endpoint, request, { headers: { Authorization: authHeader } });
        } else {
            await api.post(endpoint, request, { headers: { Authorization: authHeader } });
        }
        toast.success(isEditing ? "Policy updated" : "Policy created");
        await goto(`/teams/${id}`);
    } catch (error) {
        errorMessage = error instanceof Error ? error.message : String(error);
        toast.error(isEditing ? "Failed to update policy" : "Failed to create policy");
    } finally {
        isSaving = false;
    }
}
</script>

<h1 class="page-header">{isEditing ? "Edit Policy" : "Add Policy"}</h1>
<p class="text-sm text-gray-400 mb-6">Capabilities are explicit. Anything not selected is denied.</p>
{#if !editLoaded}<p class="text-sm text-gray-400 mb-4">Loading current policy…</p>{/if}

<form onsubmit={(event) => { event.preventDefault(); savePolicy(); }} class="flex flex-col gap-5">
    <div class="form-group">
        <label for="policyName">Policy name</label>
        <input id="policyName" type="text" maxlength="120" required bind:value={policyName} />
    </div>

    <div class="card">
        <label class="flex items-center gap-2"><input type="checkbox" bind:checked={allowSigning} /> Sign events</label>
        {#if allowSigning}
            <div class="form-group mb-0!">
                <label for="allowedKinds">Allowed event kinds</label>
                <input id="allowedKinds" type="text" bind:value={allowedKinds} placeholder="1, 7, 27235" />
            </div>
        {/if}
    </div>

    <div class="card">
        <h2 class="font-semibold">Encryption and decryption</h2>
        {#each [
            { label: "NIP-44 encrypt", enabled: nip44Encrypt, recipient: nip44EncryptRecipient, key: "nip44Encrypt" },
            { label: "NIP-44 decrypt", enabled: nip44Decrypt, recipient: nip44DecryptRecipient, key: "nip44Decrypt" },
            { label: "NIP-04 encrypt (legacy)", enabled: nip04Encrypt, recipient: nip04EncryptRecipient, key: "nip04Encrypt" },
            { label: "NIP-04 decrypt (legacy)", enabled: nip04Decrypt, recipient: nip04DecryptRecipient, key: "nip04Decrypt" },
        ] as capability}
            <div class="flex flex-col sm:flex-row sm:items-center gap-2">
                <label class="flex items-center gap-2 grow">
                    <input type="checkbox" checked={capability.enabled} onchange={(event) => {
                        const checked = event.currentTarget.checked;
                        if (capability.key === "nip44Encrypt") nip44Encrypt = checked;
                        else if (capability.key === "nip44Decrypt") nip44Decrypt = checked;
                        else if (capability.key === "nip04Encrypt") nip04Encrypt = checked;
                        else nip04Decrypt = checked;
                    }} /> {capability.label}
                </label>
                {#if capability.enabled}
                    <select value={capability.recipient} onchange={(event) => {
                        const value = event.currentTarget.value as RecipientScope;
                        if (capability.key === "nip44Encrypt") nip44EncryptRecipient = value;
                        else if (capability.key === "nip44Decrypt") nip44DecryptRecipient = value;
                        else if (capability.key === "nip04Encrypt") nip04EncryptRecipient = value;
                        else nip04DecryptRecipient = value;
                    }} aria-label={`${capability.label} counterparty`}>
                        <option value="any">Any public key</option>
                        <option value="self_only">Only the managed key</option>
                    </select>
                {/if}
            </div>
        {/each}
    </div>

    {#if errorMessage}<p class="input-error">{errorMessage}</p>{/if}
    <button type="submit" class="button button-primary self-start" disabled={isSaving || !editLoaded}>
        {isSaving ? "Saving…" : (isEditing ? "Update policy" : "Save policy")}
    </button>
</form>
