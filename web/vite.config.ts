import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import { readFileSync } from "node:fs";

const version = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")).version;

export default defineConfig({
    define: {
        __KEYCAST_VERSION__: JSON.stringify(version),
        __KEYCAST_REVISION__: JSON.stringify(process.env.KEYCAST_BUILD_REVISION || "development"),
    },
    plugins: [sveltekit()],
    build: {
        target: "esnext",
        minify: "oxc",
        rollupOptions: {
            output: {
                sanitizeFileName: (name: string) => {
                    return name.replace(/[<>*#"{}|^[\]`;?:&=+$,]/g, "_");
                },
            },
        },
    },
});
