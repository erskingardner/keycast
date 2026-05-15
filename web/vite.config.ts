import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
    plugins: [sveltekit()],
    resolve: {
        alias: {
            // NDK depends on tseep. Use tseep's safe emitter entry so the production bundle
            // does not include the default eval-based task collection.
            tseep: "tseep/lib/ee-safe.js",
        },
    },
    build: {
        target: "esnext",
        minify: "esbuild",
        rollupOptions: {
            output: {
                sanitizeFileName: (name: string) => {
                    return name.replace(/[<>*#"{}|^[\]`;?:&=+$,]/g, "_");
                },
            },
        },
    },
    esbuild: {
        charset: "utf8",
    },
});
