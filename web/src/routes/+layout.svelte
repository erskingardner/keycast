<script lang="ts">
import "../app.css";
import Header from "$lib/components/Header.svelte";
import { getCurrentUser, setCurrentUser } from "$lib/current_user.svelte";
import { initApi } from "$lib/keycast_api.svelte";
import ndk from "$lib/ndk.svelte";
import { SigninMethod, signin, signout } from "$lib/utils/auth";
import { onMount } from "svelte";
import { Toaster } from "svelte-hot-french-toast";

let { data, children } = $props();
let keycastCookie = $derived(data.keycastCookie);
initApi();

$effect(() => {
    if (keycastCookie && getCurrentUser()?.user?.pubkey !== keycastCookie) {
        setCurrentUser(keycastCookie);
    }
});

onMount(() => {
    if (!window.nostr) {
        import("nostr-login")
            .then(async ({ init }) => {
                init({
                    onAuth(npub, options) {
                        if (options.type === "logout") {
                            signout(ndk);
                        } else {
                            let user = ndk.getUser({ npub });
                            signin(
                                ndk,
                                undefined,
                                SigninMethod.NostrLogin,
                                undefined,
                                user,
                            );
                        }
                    },
                });
            })
            .catch((error) => console.log("Failed to load nostr-login", error));
    }
});
</script>

<Toaster />
<Header />

<div class="container">
	<!-- Background orbs -->
    <div class="fixed inset-0 -z-10">
        <div class="blob top-10 left-1/4 w-[600px] h-[600px] bg-purple-500/10 animate-blob"></div>
        <div class="blob top-1/2 -right-20 w-96 h-96 bg-red-500/10 animate-blob animation-delay-2000"></div>
        <div class="blob bottom-20 left-32 w-72 h-72 bg-orange-500/10 animate-blob animation-delay-5500"></div>
    </div>
	{@render children()}
</div>


<style lang="postcss">
	@keyframes blob {
        0% { transform: translate(0px, 0px) scale(1); }
        33% { transform: translate(30px, -50px) scale(1.4); }
        66% { transform: translate(-20px, 20px) scale(0.8); }
        100% { transform: translate(0px, 0px) scale(1); }
    }

	.blob {
		@apply absolute rounded-full mix-blend-multiply filter blur-3xl animate-blob;
	}
    
    .animate-blob {
        animation: blob 14s infinite;
    }
    
    .animation-delay-2000 {
        animation-delay: 2s;
    }
    
    .animation-delay-5500 {
        animation-delay: 5.5s;
    }
</style>
