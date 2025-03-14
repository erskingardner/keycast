<div class="relative min-h-screen overflow-hidden">
    <!-- Content -->
    <div class="flex flex-col items-center justify-center mt-8 md:mt-20 relative">
        <h1 class="text-4xl md:text-5xl font-extrabold">Work Together</h1>
        <h1 class="text-2xl md:text-4xl font-light text-gray-400">without losing your keys</h1>

        <!-- Feature sections -->
        <div class="grid md:grid-cols-3 gap-8 max-w-6xl mx-auto mt-20 px-6">
            <!-- Team Management -->
            <div class="feature-panel">
                <h3 class="text-xl font-bold mb-3">Manage Multiple Teams</h3>
                <p class="text-gray-400">
                    Easily manage keys across multiple teams with secure, organized access control.
                </p>
            </div>

            <!-- Custom Policies -->
            <div class="feature-panel">
                <h3 class="text-xl font-bold mb-3">Custom Permissions</h3>
                <p class="text-gray-400">
                    Create granular policies to control who can sign, encrypt, or decrypt specific events.
                </p>
            </div>

            <!-- NIP-46 Security -->
            <div class="feature-panel">
                <h3 class="text-xl font-bold mb-3">Secure Remote Signing</h3>
                <p class="text-gray-400">
                    Use NIP-46 remote signing to keep private keys encrypted and secure across all clients.
                </p>
            </div>
        </div>
    </div>
</div>

<style>
    .feature-panel::before {
        content: '';
        position: absolute;
        top: 0;
        left: -100%;
        width: 200%;
        height: 100%;
        background: linear-gradient(
            115deg,
            transparent 0%,
            transparent 40%,
            rgba(255, 255, 255, 0.04) 45%,
            rgba(255, 255, 255, 0.10) 50%,
            rgba(255, 255, 255, 0.08) 60%,
            rgba(255, 255, 255, 0.04) 75%,
            rgba(255, 255, 255, 0.02) 80%,
            transparent 85%,
            transparent 100%
        );
        pointer-events: none;
    }
</style>
