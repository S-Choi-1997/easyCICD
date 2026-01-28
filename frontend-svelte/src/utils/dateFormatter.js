import { formatDistance, format, parseISO, isValid } from 'date-fns';
import { ko } from 'date-fns/locale';

/**
 * 날짜 문자열을 파싱
 * SQLite의 datetime('now')는 UTC로 저장됨
 */
function parseDate(dateString) {
    if (!dateString) return null;

    // SQLite의 "YYYY-MM-DD HH:MM:SS" 형식을 ISO 형식으로 변환
    // datetime('now')는 UTC이므로 'Z' 접미사 추가
    const normalized = dateString.replace(' ', 'T') + 'Z';
    const date = parseISO(normalized);

    return isValid(date) ? date : null;
}

/**
 * 상대 시간 표시 (예: "3분 전", "2시간 전")
 */
export function formatRelativeTime(dateString, baseTime = new Date()) {
    if (!dateString) return '알 수 없음';

    try {
        const date = parseDate(dateString);
        if (!date) return '알 수 없음';

        return formatDistance(date, baseTime, { addSuffix: true, locale: ko });
    } catch (error) {
        console.error('Date parsing error:', error);
        return '알 수 없음';
    }
}

/**
 * 절대 시간 표시 (예: "2026-01-09 14:30:15")
 */
export function formatAbsoluteTime(dateString) {
    if (!dateString) return '알 수 없음';

    try {
        const date = parseDate(dateString);
        if (!date) return '알 수 없음';

        return format(date, 'yyyy-MM-dd HH:mm:ss');
    } catch (error) {
        console.error('Date parsing error:', error);
        return '알 수 없음';
    }
}

/**
 * 짧은 날짜 표시 (예: "01/09 14:30")
 */
export function formatShortTime(dateString) {
    if (!dateString) return '알 수 없음';

    try {
        const date = parseDate(dateString);
        if (!date) return '알 수 없음';

        return format(date, 'MM/dd HH:mm');
    } catch (error) {
        console.error('Date parsing error:', error);
        return '알 수 없음';
    }
}

/**
 * 소요 시간 계산 (시작 ~ 종료)
 */
export function formatDuration(startString, endString) {
    if (!startString) return '알 수 없음';
    if (!endString) return '진행 중';

    try {
        const start = parseDate(startString);
        const end = parseDate(endString);

        if (!start || !end) return '알 수 없음';

        const diffMs = end - start;
        const diffSec = Math.floor(diffMs / 1000);

        if (diffSec < 60) return `${diffSec}초`;

        const diffMin = Math.floor(diffSec / 60);
        if (diffMin < 60) return `${diffMin}분 ${diffSec % 60}초`;

        const diffHour = Math.floor(diffMin / 60);
        return `${diffHour}시간 ${diffMin % 60}분`;
    } catch (error) {
        console.error('Duration calculation error:', error);
        return '알 수 없음';
    }
}
