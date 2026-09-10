<script lang="ts">
    import { goto } from "$app/navigation";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type {
        Policy,
        PolicyDocument,
        TeamWithRelations,
    } from "$lib/types";
    import PolicyPermissions from "./PolicyPermissions.svelte";
    import { buildDocument, createEditorState } from "$lib/policy/editor";
    import { toast } from "svelte-hot-french-toast";

    let {
        id,
        policy: initialPolicy,
        policyId,
        onSaved,
    }: {
        id: string;
        policy?: Policy;
        policyId?: number;
        onSaved?: () => void | Promise<void>;
    } = $props();
    const api = new KeycastApi();
    const user = $derived(getCurrentUser()?.user);
    const editPolicyId = $derived(initialPolicy?.id ?? policyId ?? 0);
    const isEditing = $derived(
        Number.isInteger(editPolicyId) && editPolicyId > 0,
    );

    let policyName = $state("");
    let permissions = $state(createEditorState());
    let errorMessage: string | null = $state(null);
    let isSaving = $state(false);
    let editLoadStarted = $state(false);
    let editLoaded = $state(false);

    $effect(() => {
        if (editLoadStarted) return;
        if (initialPolicy) {
            editLoadStarted = true;
            try {
                loadPolicy(initialPolicy);
                editLoaded = true;
            } catch (error) {
                errorMessage = error instanceof Error ? error.message : String(error);
            }
            return;
        }
        if (!isEditing) {
            editLoaded = true;
            return;
        }
        if (!user?.pubkey) return;
        editLoadStarted = true;
        const endpoint = `/teams/${id}`;
        api.buildAuthHeader(endpoint, "GET", user.pubkey)
            .then((authorization) =>
                api.get<TeamWithRelations>(endpoint, {
                    headers: { Authorization: authorization },
                }),
            )
            .then((team) => {
                const policy = team.policies.find(
                    (item) => item.id === editPolicyId,
                );
                if (!policy) throw new Error("Policy not found");
                loadPolicy(policy);
                editLoaded = true;
            })
            .catch((error) => {
                errorMessage =
                    error instanceof Error ? error.message : String(error);
                editLoaded = false;
            });
    });

    function loadPolicy(policy: Policy) {
        policyName = policy.name;
        permissions = createEditorState(policy.document);
    }

    async function savePolicy() {
        if (!user?.pubkey || isSaving || !editLoaded) return;
        errorMessage = null;
        let document: PolicyDocument;
        try {
            document = buildDocument(permissions);
        } catch (error) {
            errorMessage = error instanceof Error ? error.message : String(error);
            return;
        }

        const request = {
            name: policyName,
            document,
        };
        isSaving = true;
        try {
            const body = JSON.stringify(request);
            const endpoint = isEditing
                ? `/teams/${id}/policies/${editPolicyId}`
                : `/teams/${id}/policies`;
            const method = isEditing ? "PUT" : "POST";
            const authHeader = await api.buildAuthHeader(
                endpoint,
                method,
                user.pubkey,
                body,
            );
            if (isEditing) {
                await api.put(endpoint, request, {
                    headers: { Authorization: authHeader },
                });
            } else {
                await api.post(endpoint, request, {
                    headers: { Authorization: authHeader },
                });
            }
            toast.success(isEditing ? "Policy updated" : "Policy created");
            if (onSaved) await onSaved();
            else await goto(`/teams/${id}`);
        } catch (error) {
            errorMessage =
                error instanceof Error ? error.message : String(error);
            toast.error(
                isEditing
                    ? "Failed to update policy"
                    : "Failed to create policy",
            );
        } finally {
            isSaving = false;
        }
    }
</script>

<h2 class="page-header">{isEditing ? "Edit Policy" : "Add Policy"}</h2>
<p class="text-sm text-muted mb-6">
    Capabilities are explicit. Anything not selected is denied.
</p>
{#if !editLoaded && !errorMessage}<p class="text-sm text-muted mb-4">
        Loading current policy…
    </p>{/if}

<form
    onsubmit={(event) => {
        event.preventDefault();
        savePolicy();
    }}
    class="flex flex-col gap-5"
>
    <div class="form-group">
        <label for="policyName">Policy name</label>
        <input
            id="policyName"
            type="text"
            maxlength="120"
            required
            bind:value={policyName}
        />
    </div>

    <fieldset disabled={isSaving || !editLoaded} class="min-w-0">
        <legend class="sr-only">Policy permissions</legend>
        <PolicyPermissions bind:value={permissions} />
    </fieldset>

    {#if errorMessage}<p class="input-error" role="alert">{errorMessage}</p>{/if}
    <button
        type="submit"
        class="button button-primary self-start"
        disabled={isSaving || !editLoaded}
    >
        {isSaving ? "Saving…" : isEditing ? "Update policy" : "Save policy"}
    </button>
</form>
