<script lang="ts">
    import "../app.css";
    import Header from "$lib/components/Header.svelte";
    import { getCurrentUser, setCurrentUser } from "$lib/current_user.svelte";
    import { initApi } from "$lib/keycast_api.svelte";
    import { Toaster } from "svelte-hot-french-toast";

    let { data, children } = $props();
    let keycastCookie = $derived(data.keycastCookie);
    initApi();

    $effect(() => {
        if (keycastCookie && getCurrentUser()?.user?.pubkey !== keycastCookie) {
            setCurrentUser(keycastCookie);
        }
    });
</script>

<Toaster
    toastOptions={{
        style: "background: #252c29; color: #fcfdf8; border-radius: 3px; font-size: 13px;",
    }}
/>
<Header />
<main id="main-content" class="app-main">{@render children()}</main>
