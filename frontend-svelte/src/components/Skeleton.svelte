<script>
    export let type = 'project-card'; // 'project-card' | 'build-item' | 'build-list' | 'text' | 'button'
    export let count = 1;
</script>

{#if type === 'project-card'}
    {#each Array(count) as _, i}
        <div class="skeleton-project-card">
            <div class="skeleton-header">
                <div class="skeleton-title"></div>
                <div class="skeleton-badge"></div>
            </div>
            <div class="skeleton-info">
                <div class="skeleton-line"></div>
                <div class="skeleton-line"></div>
                <div class="skeleton-line short"></div>
            </div>
            <div class="skeleton-actions">
                <div class="skeleton-button"></div>
                <div class="skeleton-button"></div>
                <div class="skeleton-button"></div>
            </div>
        </div>
    {/each}
{:else if type === 'build-item'}
    {#each Array(count) as _, i}
        <div class="skeleton-build-item">
            <div class="skeleton-badge"></div>
            <div class="skeleton-content">
                <div class="skeleton-line"></div>
                <div class="skeleton-line short"></div>
            </div>
        </div>
    {/each}
{:else if type === 'build-list'}
    {#each Array(count) as _, i}
        <div class="skeleton-build-list-item">
            <div class="skeleton-build-info">
                <div class="skeleton-badge small"></div>
                <div class="skeleton-badge small"></div>
                <div class="skeleton-line medium"></div>
            </div>
            <div class="skeleton-line short"></div>
        </div>
    {/each}
{:else if type === 'text'}
    <div class="skeleton-line"></div>
{:else if type === 'button'}
    <div class="skeleton-button"></div>
{/if}

<style>
    @keyframes shimmer {
        0% {
            background-position: -1000px 0;
        }
        100% {
            background-position: 1000px 0;
        }
    }

    .skeleton-project-card,
    .skeleton-build-item,
    .skeleton-build-list-item,
    .skeleton-line,
    .skeleton-button,
    .skeleton-title,
    .skeleton-badge {
        background: linear-gradient(
            90deg,
            #f0f0f0 0%,
            #e0e0e0 50%,
            #f0f0f0 100%
        );
        background-size: 1000px 100%;
        animation: shimmer 2s infinite;
        border-radius: 4px;
    }

    .skeleton-project-card {
        background: white;
        border: 1px solid var(--gray-200);
        border-radius: 8px;
        padding: 1.5rem;
        margin-bottom: 1rem;
    }

    .skeleton-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
    }

    .skeleton-title {
        width: 200px;
        height: 24px;
    }

    .skeleton-badge {
        width: 80px;
        height: 24px;
    }

    .skeleton-info {
        margin-bottom: 1rem;
    }

    .skeleton-line {
        height: 16px;
        margin-bottom: 0.5rem;
        width: 100%;
    }

    .skeleton-line.short {
        width: 60%;
    }

    .skeleton-actions {
        display: flex;
        gap: 0.5rem;
    }

    .skeleton-button {
        width: 80px;
        height: 32px;
    }

    .skeleton-build-item {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 1rem;
        border-bottom: 1px solid var(--gray-200);
    }

    .skeleton-content {
        flex: 1;
    }

    .skeleton-build-list-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.75rem 1rem;
        background: white;
        border-bottom: 1px solid var(--gray-200);
    }

    .skeleton-build-info {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        flex: 1;
    }

    .skeleton-badge.small {
        width: 60px;
        height: 20px;
    }

    .skeleton-line.medium {
        width: 200px;
        height: 16px;
    }

    .skeleton-line.short {
        width: 80px;
    }
</style>
