<script lang="ts">
    import { goto } from "$app/navigation";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    let { id, onSaved }: { id: string; onSaved?: () => void | Promise<void> } =
        $props();
    const api = new KeycastApi();
    let name = $state("");
    let secretKey = $state("");
    let error = $state("");
    let busy = $state(false);
    async function submit() {
        const user = getCurrentUser()?.user;
        if (!user || busy) return;
        error = "";
        busy = true;
        const request = { name: name.trim(), secret_key: secretKey };
        secretKey = "";
        try {
            const endpoint = `/teams/${id}/keys`;
            const authorization = await api.buildAuthHeader(
                endpoint,
                "POST",
                user.pubkey,
                JSON.stringify(request),
            );
            await api.post(endpoint, request, {
                headers: { Authorization: authorization },
            });
            if (onSaved) await onSaved();
            else await goto(`/teams/${id}`);
        } catch (e) {
            error = e instanceof Error ? e.message : "Key import failed";
        } finally {
            request.secret_key = "";
            secretKey = "";
            busy = false;
        }
    }
</script>

<h2 class="page-header">Import a key</h2>
<p class="description mb-5">
    The trusted CLI on your server is the safer import method. Browser import
    gives this page access to your private key.
</p>
<form
    onsubmit={(event) => {
        event.preventDefault();
        void submit();
    }}
>
    <div class="grid sm:grid-cols-2 gap-4">
        <div class="form-group">
            <label for="key-name">Key name</label><input
                id="key-name"
                type="text"
                required
                maxlength="120"
                bind:value={name}
                placeholder="Personal identity"
            />
        </div>
        <div class="form-group">
            <label for="key-secret">Private key · nsec or hex</label><input
                id="key-secret"
                type="password"
                required
                autocomplete="one-time-code"
                data-1p-ignore
                data-lpignore="true"
                data-bwignore
                spellcheck="false"
                bind:value={secretKey}
                placeholder="nsec1…"
            />
        </div>
    </div>
    {#if error}<p role="alert" class="input-error">{error}</p>{/if}
    <button class="button button-primary" disabled={busy}
        >{busy ? "Awaiting approval…" : "Import key"}</button
    >
</form>
