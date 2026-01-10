/**
 * 커밋 해시를 짧은 형태로 변환 (7자)
 */
export function formatCommitHash(hash) {
    if (!hash || hash === 'head') return '알 수 없음';
    return hash.substring(0, 7);
}

/**
 * 커밋 메시지를 짧게 자르기 (최대 50자)
 */
export function formatCommitMessage(message) {
    if (!message) return '커밋 메시지 없음';

    // 첫 줄만 추출
    const firstLine = message.split('\n')[0].trim();

    if (firstLine.length <= 50) return firstLine;
    return firstLine.substring(0, 47) + '...';
}

/**
 * 작성자 이름 포맷팅
 */
export function formatAuthor(author) {
    if (!author) return '알 수 없음';

    // 이메일 제거 (예: "John Doe <john@example.com>" -> "John Doe")
    const match = author.match(/^([^<]+)/);
    return match ? match[1].trim() : author;
}

/**
 * GitHub 커밋 URL 생성
 */
export function getCommitUrl(repo, hash) {
    if (!repo || !hash || hash === 'head') return null;

    // repo: "https://github.com/user/repo.git" -> "user/repo"
    const match = repo.match(/github\.com[/:]([\w-]+\/[\w-]+)/);
    if (!match) return null;

    const repoPath = match[1].replace('.git', '');
    return `https://github.com/${repoPath}/commit/${hash}`;
}
