import { writable, derived } from 'svelte/store';

// WebSocket 연결 상태
export const wsConnected = writable(false);
export const wsInstance = writable(null);

// 구독 관리
const subscriptions = writable(new Map());

// WebSocket 메시지 스트림
const messages = writable([]);

/**
 * WebSocket 초기화 및 연결
 */
export function initWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;

    const ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        console.log('[WebSocket] Connected');
        wsConnected.set(true);
        wsInstance.set(ws);
    };

    ws.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            messages.update(msgs => [...msgs, data]);

            // 구독자에게 메시지 전달
            subscriptions.update(subs => {
                subs.forEach((callback, key) => {
                    callback(data);
                });
                return subs;
            });
        } catch (error) {
            console.error('[WebSocket] Message parsing error:', error);
        }
    };

    ws.onerror = (error) => {
        console.error('[WebSocket] Error:', error);
    };

    ws.onclose = () => {
        console.log('[WebSocket] Disconnected, reconnecting in 3s...');
        wsConnected.set(false);
        wsInstance.set(null);

        // 3초 후 재연결
        setTimeout(initWebSocket, 3000);
    };

    return ws;
}

/**
 * WebSocket 메시지 구독
 * @param {string} key - 구독 키 (고유 식별자)
 * @param {function} callback - 메시지 콜백
 * @returns {function} unsubscribe 함수
 */
export function subscribe(key, callback) {
    subscriptions.update(subs => {
        subs.set(key, callback);
        return subs;
    });

    // unsubscribe 함수 반환
    return () => {
        subscriptions.update(subs => {
            subs.delete(key);
            return subs;
        });
    };
}

/**
 * WebSocket으로 메시지 전송
 */
export function sendMessage(message) {
    wsInstance.subscribe(ws => {
        if (ws && ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify(message));
        }
    })();
}
