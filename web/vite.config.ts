import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import replace from 'vite-plugin-filter-replace';

const { VITE_API_DOMAIN } = process.env;

export default defineConfig({
    plugins: [
        //TODO: eliminate need for this.
        replace(
            [
              {
                filter: /\.css$/,
                replace: {
                  from: /__VITE_API_DOMAIN__/g,
                  to: VITE_API_DOMAIN || "http://localhost:3000",
                },
              },
            ]
        ),
        sveltekit()
    ],
});
