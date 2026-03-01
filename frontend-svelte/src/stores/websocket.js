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
        wsConnected.set(true);
        wsInstance.set(ws);
    };

    ws.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            messages.update(msgs => [...msgs, data]);

            subscriptions.update(subs => {
                subs.forEach((callback) => callback(data));
                return subs;
            });
        } catch (error) {
            console.error('[WebSocket] 메시지 파싱 오류:', error);
        }
    };

    ws.onerror = () => {};

    ws.onclose = () => {
        wsConnected.set(false);
        wsInstance.set(null);

        let currentSubscriptions;
        subscriptions.update(subs => {
            currentSubscriptions = new Map(subs);
            return subs;
        });

        setTimeout(() => {
            const newWs = initWebSocket();
            newWs.addEventListener('open', () => {
                subscriptions.set(currentSubscriptions);
            }, { once: true });
        }, 3000);
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
