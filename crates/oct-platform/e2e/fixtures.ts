import { test as base, type Page, type TestInfo } from '@playwright/test';
import { spawn, type ChildProcess } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import net from 'net';

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

type ServerInfo = { url: string, workDir: string };

const spawnServer = async (env: NodeJS.ProcessEnv, onLog: (msg: string) => void): Promise<{ process: ChildProcess, info: ServerInfo }> => {
    const port = await getFreePort();
    if (process.env.VERBOSE) {
        console.log(`Starting server on port ${port}`);
    }

    const args = ['run', '-p', 'oct-platform'];
    if (process.env.VERBOSE) {
        args.push('--', '--verbose');
    }

    const serverProcess = spawn('cargo', args, {
      env: {
        ...process.env,
        OCT_PLATFORM_MOCK: 'true',
        OCT_PLATFORM_PORT: port.toString(),
        ...env
      },
      cwd: path.resolve(__dirname, '../../..'),
      stdio: 'pipe',
    });

    await new Promise<void>((resolve, reject) => {
      serverProcess.stdout?.on('data', (data) => {
        const output = data.toString();
        onLog(`[Server ${port}]: ${output}`);
        if (output.includes(`Listening on http://0.0.0.0:${port}`)) {
          resolve();
        }
      });
      serverProcess.stderr?.on('data', (data) => {
        onLog(`[Server ${port} ERR]: ${data.toString()}`);
      });
      serverProcess.on('error', reject);
      serverProcess.on('exit', (code) => {
        if (code !== null && code !== 0 && code !== 137 && code !== 143) {
             reject(new Error(`Server exited with code ${code}`));
        }
      });
    });

    return {
        process: serverProcess,
        info: { url: `http://127.0.0.1:${port}`, workDir: '' }
    };
};

type UiFixtures = {
  server: { url: string };
  workspaceServer: { url: string };
};

export const test = base.extend<UiFixtures>({
  server: async ({ }, use, testInfo) => {
    const logs: string[] = [];
    const onLog = (msg: string) => {
        logs.push(msg);
        if (process.env.VERBOSE) console.log(msg);
    };

    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oct-platform-test-'));
    const configPath = path.join(tempDir, 'oct.toml');

    const { process: srvProcess, info } = await spawnServer({
        OCT_CONFIG_PATH: configPath,
    }, onLog);

    try {
        await use(info);
    } catch (e) {
        console.log(`\n=== Logs for failed test: ${testInfo.title} ===\n`);
        console.log(logs.join(''));
        console.log(`\n=== End Logs ===\n`);
        throw e;
    } finally {
        srvProcess.kill();
        fs.rmSync(tempDir, { recursive: true, force: true });
    }
  },

  workspaceServer: async ({ }, use, testInfo) => {
    const logs: string[] = [];
    const onLog = (msg: string) => {
        logs.push(msg);
        if (process.env.VERBOSE) console.log(msg);
    };

    const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), 'oct-platform-workspace-'));

    const { process: srvProcess, info } = await spawnServer({
        OCT_WORKSPACE_ROOT: tempHome,
        OCT_CONFIG_PATH: '', // Ensure FileConfigManager is NOT used
    }, onLog);

    try {
        await use(info);
    } catch (e) {
        console.log(`\n=== Logs for failed test: ${testInfo.title} ===\n`);
        console.log(logs.join(''));
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


export { expect } from '@playwright/test';
