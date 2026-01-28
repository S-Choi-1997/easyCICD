<script>
  import { onMount, onDestroy } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import '@xterm/xterm/css/xterm.css';

  export let containerId;
  export let onClose = () => {};

  let terminalElement;
  let terminal;
  let fitAddon;
  let ws;
  let connected = false;
  let error = null;
  let inputBuffer = '';  // 입력 버퍼

  onMount(() => {
    terminal = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#d4d4d4',
      },
    });

    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

    terminal.open(terminalElement);

    setTimeout(() => {
      fitAddon.fit();
      terminal.focus();
    }, 0);

    connectWebSocket();

    const resizeObserver = new ResizeObserver(() => {
      if (fitAddon && terminal) {
        fitAddon.fit();
        sendResize();
      }
    });
    resizeObserver.observe(terminalElement);

    terminal.onData((data) => {
      if (!ws || ws.readyState !== WebSocket.OPEN) return;

      // Enter 키 처리
      if (data === '\r' || data === '\n') {
        terminal.write('\r\n');  // 로컬 줄바꿈
        if (inputBuffer.length > 0) {
          ws.send(JSON.stringify({ type: 'input', data: inputBuffer + '\n' }));
          inputBuffer = '';
        } else {
          // 빈 엔터도 전송 (프롬프트 갱신용)
          ws.send(JSON.stringify({ type: 'input', data: '\n' }));
        }
      }
      // Backspace 처리
      else if (data === '\x7f' || data === '\b') {
        if (inputBuffer.length > 0) {
          inputBuffer = inputBuffer.slice(0, -1);
          terminal.write('\b \b');  // 로컬에서 지우기
        }
      }
      // Ctrl+C 처리
      else if (data === '\x03') {
        inputBuffer = '';
        ws.send(JSON.stringify({ type: 'input', data: '\x03' }));
      }
      // 일반 문자
      else {
        inputBuffer += data;
        terminal.write(data);  // 로컬 에코
      }
    });

    return () => {
      resizeObserver.disconnect();
    };
  });

  onDestroy(() => {
    if (ws) {
      ws.close();
    }
    if (terminal) {
      terminal.dispose();
    }
  });

  function connectWebSocket() {
    error = null;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/containers/${containerId}/terminal`;

    terminal.writeln('Connecting to container...\r\n');

    ws = new WebSocket(wsUrl);

    ws.onopen = () => {
      console.log('[Terminal] WebSocket connected');
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg.type === 'output') {
          terminal.write(msg.data);
        } else if (msg.type === 'error') {
          terminal.writeln(`\r\n\x1b[31mError: ${msg.message}\x1b[0m\r\n`);
          error = msg.message;
          connected = false;
        } else if (msg.type === 'connected') {
          connected = true;
          terminal.writeln('\x1b[32mConnected!\x1b[0m\r\n');
          sendResize();
          terminal.focus();
        }
      } catch (e) {
        terminal.write(event.data);
      }
    };

    ws.onerror = (err) => {
      console.error('[Terminal] WebSocket error:', err);
      terminal.writeln('\r\n\x1b[31mConnection error\x1b[0m\r\n');
      error = 'Connection error';
    };

    ws.onclose = () => {
      connected = false;
      terminal.writeln('\r\n\x1b[33mConnection closed\x1b[0m\r\n');
    };
  }

  function sendResize() {
    if (ws && ws.readyState === WebSocket.OPEN && terminal) {
      ws.send(JSON.stringify({
        type: 'resize',
        rows: terminal.rows,
        cols: terminal.cols,
      }));
    }
  }

  function reconnect() {
    if (ws) {
      ws.close();
    }
    terminal.clear();
    connectWebSocket();
    terminal.focus();
  }
</script>

<div class="terminal-container">
  <div class="terminal-header">
    <span class="terminal-title">Terminal (Container #{containerId})</span>
    <div class="terminal-status">
      {#if connected}
        <span class="status-dot connected"></span>
        <span>Connected</span>
      {:else}
        <span class="status-dot disconnected"></span>
        <span>Disconnected</span>
        <button class="btn-reconnect" on:click={reconnect}>Reconnect</button>
      {/if}
    </div>
    <button class="btn-close" on:click={onClose}>X</button>
  </div>
  <div class="terminal-body" bind:this={terminalElement}></div>
</div>

<style>
  .terminal-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #1e1e1e;
    border-radius: 0.5rem;
    overflow: hidden;
    border: 1px solid #3c3c3c;
  }

  .terminal-header {
    display: flex;
    align-items: center;
    padding: 0.5rem 1rem;
    background: #252526;
    border-bottom: 1px solid #3c3c3c;
    gap: 1rem;
  }

  .terminal-title {
    color: #d4d4d4;
    font-weight: 600;
    font-size: 0.875rem;
    flex: 1;
  }

  .terminal-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #d4d4d4;
    font-size: 0.75rem;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .status-dot.connected {
    background: #10b981;
  }

  .status-dot.disconnected {
    background: #ef4444;
  }

  .btn-reconnect {
    background: #3b82f6;
    border: none;
    color: white;
    padding: 0.25rem 0.5rem;
    border-radius: 0.25rem;
    cursor: pointer;
    font-size: 0.75rem;
  }

  .btn-reconnect:hover {
    background: #2563eb;
  }

  .btn-close {
    background: none;
    border: none;
    color: #9ca3af;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    font-size: 1rem;
    line-height: 1;
  }

  .btn-close:hover {
    color: #ef4444;
  }

  .terminal-body {
    flex: 1;
    padding: 0.5rem;
    min-height: 300px;
  }

  .terminal-body :global(.xterm) {
    height: 100%;
  }

  .terminal-body :global(.xterm-viewport) {
    overflow-y: auto !important;
  }
</style>
