// biome-ignore lint/style/useNodejsImportProtocol: We're using Bun
import { existsSync } from "fs";
// biome-ignore lint/style/useNodejsImportProtocol: We're using Bun
import { join } from "path";
// biome-ignore lint/style/useNodejsImportProtocol: We're using Bun
import { chmod } from "fs/promises";

class FileKeyManager {
    async generateMasterKey(): Promise<void> {
        try {
            // Generate 32 random bytes using Bun's crypto API
            const key = crypto.getRandomValues(new Uint8Array(32));

            // Convert to base64
            const encoded = btoa(String.fromCharCode(...key));

            // Get project root directory
            const projectRoot = `${import.meta.dir}/..`;
            const keyPath = `${projectRoot}/master.key`;

            // Write the key to file using Bun's file API
            await Bun.write(keyPath, encoded);

            // Set file permissions to 600 (read/write for owner only)
            await chmod(keyPath, 0o600);

            console.log(`Saved new master key to ${keyPath}`);
        } catch (error) {
            throw new Error(`Failed to generate master key: ${error.message}`);
        }
    }
}

async function main() {
    try {
        // Check if key already exists
        const keyPath = join(import.meta.dir, "..", "master.key");
        if (existsSync(keyPath)) {
            const force = process.argv.includes("--force");
            if (!force) {
                console.error("Master key already exists. Use --force to overwrite.");
                process.exit(1);
            }
            console.warn("Warning: Overwriting existing master key!");
        }

        const keyManager = new FileKeyManager();
        await keyManager.generateMasterKey();
        console.log("✅ Master key generated successfully!");
        process.exit(0);
    } catch (error) {
        console.error("❌ Failed to generate key:", error.message);
        process.exit(1);
    }
}

main(); 
