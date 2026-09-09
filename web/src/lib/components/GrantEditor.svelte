<script lang="ts">
    import Copy from "$lib/components/Copy.svelte";
    import Loader from "$lib/components/Loader.svelte";
    import PolicyCard from "$lib/components/PolicyCard.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type {
        GrantCreationResponse,
        Policy,
        StoredKey,
        Team,
        TeamWithRelations,
    } from "$lib/types";
    import { X } from "phosphor-svelte";
    import { toast } from "svelte-hot-french-toast";

    let {
        id,
        pubkey,
        onDone,
    }: { id: string; pubkey: string; onDone?: () => void | Promise<void> } =
        $props();
    // A crafted link can put anything in the route parameter, which would then be
    // signed into the `u` tag of an approval the operator is asked to confirm.
    const invalidPubkey = $derived(!/^[0-9a-f]{64}$/.test(pubkey));
    const api = new KeycastApi();
    const user = $derived(getCurrentUser()?.user);
    let isLoading = $state(true);
    let team: Team | null = $state(null);
    let policies: Policy[] = $state([]);
    let key: StoredKey | undefined = $state();
    let selectedPolicyId: number | null = $state(null);
    let grantName = $state("");
    let expiresAt = $state("");
    let invitationHours = $state(24);
    let bunkerUri: string | null = $state(null);
    let errorMessage: string | null = $state(null);
    let isSaving = $state(false);

    $effect(() => {
        if (!user?.pubkey || !isLoading || invalidPubkey) return;
        api.buildAuthHeader(`/teams/${id}`, "GET", user.pubkey)
            .then((authorization) =>
                api.get<TeamWithRelations>(`/teams/${id}`, {
                    headers: { Authorization: authorization },
                }),
            )
            .then((response) => {
                team = response.team;
                policies = response.policies;
                key = response.stored_keys.find(
                    (candidate) => candidate.public_key === pubkey,
                );
            })
            .catch((error) => {
                errorMessage =
                    error instanceof Error ? error.message : String(error);
            })
            .finally(() => {
                isLoading = false;
            });
    });

    async function createGrant() {
        if (!user?.pubkey || !selectedPolicyId || isSaving) return;
        const now = Math.floor(Date.now() / 1000);
        const request = {
            name: grantName,
            policy_id: selectedPolicyId,
            expires_at: expiresAt
                ? Math.floor(new Date(expiresAt).getTime() / 1000)
                : null,
            invitation_expires_at: now + invitationHours * 3600,
        };
        errorMessage = null;
        isSaving = true;
        try {
            const endpoint = `/teams/${id}/keys/${pubkey}/grants`;
            const body = JSON.stringify(request);
            const authorization = await api.buildAuthHeader(
                endpoint,
                "POST",
                user.pubkey,
                body,
            );
            const response = await api.post<GrantCreationResponse>(
                endpoint,
                request,
                { headers: { Authorization: authorization } },
            );
            bunkerUri = response.bunker_uri;
            toast.success("Grant and one-time invitation created");
        } catch (error) {
            errorMessage =
                error instanceof Error ? error.message : String(error);
        } finally {
            isSaving = false;
        }
    }
</script>

{#if invalidPubkey}
    <div role="alert">
        <p class="input-error">
            That is not a valid Nostr public key. Start from your workspace
            instead of following the link.
        </p>
    </div>
{:else if isLoading}
    <Loader />
{:else if bunkerUri}
    <h2 class="page-header">Invitation ready</h2>
    <div class="card max-w-3xl">
        <p class="text-warning">
            This bunker URL is shown once. Copy it now; Keycast stores only a
            hash of its secret.
        </p>
        <div
            class="flex gap-2 items-center font-mono text-xs break-all bg-canvas p-3 rounded-sm"
        >
            <span class="grow">{bunkerUri}</span><Copy value={bunkerUri} />
        </div>
        {#if onDone}<button
                type="button"
                class="button button-primary self-start"
                onclick={onDone}>Done</button
            >{:else}<a
                href={`/teams/${id}/keys/${pubkey}`}
                class="button button-primary self-start">Done</a
            >{/if}
    </div>
{:else}
    <h2 class="page-header">Connect an app</h2>
    <p class="description mb-5">
        Create a grant for {key?.name ?? "this key"}, then pair your app with
        its one-time invitation.
    </p>
    <form
        onsubmit={(event) => {
            event.preventDefault();
            createGrant();
        }}
        class="flex flex-col gap-5"
    >
        <div class="form-group">
            <label for="grantName">Client or purpose</label>
            <input
                id="grantName"
                type="text"
                required
                maxlength="120"
                bind:value={grantName}
                placeholder="White Noise on phone"
            />
        </div>
        <div class="form-group">
            <label for="expiresAt">Grant expiration (optional)</label>
            <div class="flex gap-2 items-center">
                <input
                    id="expiresAt"
                    type="datetime-local"
                    bind:value={expiresAt}
                />
                {#if expiresAt}<button
                        type="button"
                        aria-label="Clear grant expiration"
                        onclick={() => (expiresAt = "")}><X size={16} /></button
                    >{/if}
            </div>
        </div>
        <div class="form-group">
            <label for="invitationHours">Invitation validity in hours</label>
            <input
                id="invitationHours"
                type="number"
                min="1"
                max="168"
                bind:value={invitationHours}
                required
            />
        </div>
        <div>
            <h2 class="page-subheader">Policy</h2>
            {#if policies.length === 0}
                <p class="text-muted">
                    Create a policy before creating a grant.
                </p>
            {:else}
                <div class="flex flex-col gap-2">
                    {#each policies as policy}
                        <label
                            class="flex items-start gap-3 p-3 border border-line bg-surface"
                        >
                            <input
                                class="mt-1"
                                type="radio"
                                name="grant-policy"
                                value={policy.id}
                                bind:group={selectedPolicyId}
                            />
                            <div class="min-w-0 grow">
                                <PolicyCard {policy} />
                            </div>
                        </label>
                    {/each}
                </div>
            {/if}
        </div>
        {#if errorMessage}<p class="input-error">{errorMessage}</p>{/if}
        <button
            type="submit"
            class="button button-primary self-start"
            disabled={!selectedPolicyId || isSaving}
        >
            {isSaving ? "Creating…" : "Create grant"}
        </button>
    </form>
{/if}
