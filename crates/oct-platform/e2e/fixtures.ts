import { type ChildProcess, spawn } from "node:child_process";
import * as fs from "node:fs";
import net from "node:net";
import * as os from "node:os";
import * as path from "node:path";
import { test as base } from "@playwright/test";

// Helper to find a free port
const getFreePort = (): Promise<number> => {
    return new Promise((resolve) => {
        const srv = net.createServer();
        srv.listen(0, () => {
            const port = (srv.address() as net.AddressInfo).port;
            srv.close(() => resolve(port));
        });
    });
};

type ServerInfo = { url: string; workDir: string };

const spawnServer = async (
    env: NodeJS.ProcessEnv,
    onLog: (msg: string) => void,
): Promise<{ process: ChildProcess; info: ServerInfo }> => {
    const port = await getFreePort();
    if (process.env.VERBOSE) {
        console.log(`Starting server on port ${port}`);
    }

    const args = ["run", "-p", "oct-platform"];
    if (process.env.VERBOSE) {
        args.push("--", "--verbose");
    }

    const serverProcess = spawn("cargo", args, {
        env: {
            ...process.env,
            OCT_PLATFORM_MOCK: "true",
            OCT_PLATFORM_PORT: port.toString(),
            ...env,
        },
        cwd: path.resolve(__dirname, "../../.."),
        stdio: "pipe",
    });

    await new Promise<void>((resolve, reject) => {
        let settled = false;

        const cleanup = () => {
            if (settled) return;
            settled = true;
            clearTimeout(timeout);
            serverProcess.stdout?.off("data", onStdout);
            serverProcess.stderr?.off("data", onStderr);
            serverProcess.off("error", onError);
            serverProcess.off("exit", onExit);
        };

        const onStdout = (data: Buffer) => {
            const output = data.toString();
            onLog(`[Server ${port}]: ${output}`);
            if (output.includes(`Listening on http://0.0.0.0:${port}`)) {
                cleanup();
                resolve();
            }
        };

        const onStderr = (data: Buffer) => {
            onLog(`[Server ${port} ERR]: ${data.toString()}`);
        };

        const onError = (err: Error) => {
            cleanup();
            serverProcess.kill();
            reject(err);
        };

        const onExit = (code: number | null) => {
            if (code !== null && code !== 0 && code !== 137 && code !== 143) {
                cleanup();
                reject(new Error(`Server exited with code ${code}`));
            }
        };

        const timeout = setTimeout(() => {
            cleanup();
            serverProcess.kill();
            reject(new Error("Server failed to start within 30s"));
        }, 30000);

        serverProcess.stdout?.on("data", onStdout);
        serverProcess.stderr?.on("data", onStderr);
        serverProcess.on("error", onError);
        serverProcess.on("exit", onExit);
    });

    return {
        process: serverProcess,
        info: { url: `http://127.0.0.1:${port}`, workDir: "" },
    };
};

type UiFixtures = {
    server: { url: string };
    workspaceServer: { url: string };
};

export const test = base.extend<UiFixtures>({
    // biome-ignore lint/correctness/noEmptyPattern: Playwright fixture signature
    server: async ({}, use, testInfo) => {
        const logs: string[] = [];
        const onLog = (msg: string) => {
            logs.push(msg);
            if (process.env.VERBOSE) console.log(msg);
        };

        const tempDir = fs.mkdtempSync(
            path.join(os.tmpdir(), "oct-platform-test-"),
        );
        const configPath = path.join(tempDir, "oct.toml");

        const { process: srvProcess, info } = await spawnServer(
            {
                OCT_CONFIG_PATH: configPath,
            },
            onLog,
        );

        try {
            await use(info);
        } catch (e) {
            console.log(`\n=== Logs for failed test: ${testInfo.title} ===\n`);
            console.log(logs.join(""));
            console.log(`\n=== End Logs ===\n`);
            throw e;
        } finally {
            srvProcess.kill();
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    },

    // biome-ignore lint/correctness/noEmptyPattern: Playwright fixture signature
    workspaceServer: async ({}, use, testInfo) => {
        const logs: string[] = [];
        const onLog = (msg: string) => {
            logs.push(msg);
            if (process.env.VERBOSE) console.log(msg);
        };

        const tempHome = fs.mkdtempSync(
            path.join(os.tmpdir(), "oct-platform-workspace-"),
        );

        const { process: srvProcess, info } = await spawnServer(
            {
                OCT_WORKSPACE_ROOT: tempHome,
                OCT_CONFIG_PATH: "", // Ensure FileConfigManager is NOT used
            },
            onLog,
        );

        try {
            await use(info);
        } catch (e) {
            console.log(`\n=== Logs for failed test: ${testInfo.title} ===\n`);
            console.log(logs.join(""));
            console.log(`\n=== End Logs ===\n`);
            throw e;
        } finally {
            srvProcess.kill();
            fs.rmSync(tempHome, { recursive: true, force: true });
        }
    },

    // Override page to navigate to our server
    page: async ({ page, server }, use) => {
        await page.goto(`${server.url}/projects/default`);
        await use(page);
    },
});

export { expect } from "@playwright/test";
