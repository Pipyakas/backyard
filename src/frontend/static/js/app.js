// Backyard - Llama.cpp Launcher JavaScript

let selectedModelId = null;
let selectedFiles = [];
let modelsBasePath = '/models';
let expandedGroups = new Set();

// Tab to path mapping
const TAB_PATHS = {
    'dashboard': '/',
    'models': '/models',
    'servers': '/servers',
    'benchmarks': '/benchmarks',
    'settings': '/settings'
};

// Path to tab mapping
const PATH_TABS = {
    '/': 'dashboard',
    '/models': 'models',
    '/servers': 'servers',
    '/benchmarks': 'benchmarks',
    '/settings': 'settings'
};

// Load content based on URL (for pushState navigation)
function loadTabContent(path) {
    const tabId = PATH_TABS[path] || 'dashboard';

    // Update active tab
    document.querySelectorAll('.tab').forEach(t => {
        const href = TAB_PATHS[t.dataset.tab];
        if (href === path || (path === '/' && href === '/')) {
            t.classList.add('active');
        } else {
            t.classList.remove('active');
        }
    });

    // Show/hide content sections
    document.querySelectorAll('.tab-content').forEach(section => {
        section.classList.remove('active');
    });

    const section = document.getElementById(tabId);
    if (section) section.classList.add('active');
}

// Navigation helper
function navigateTo(path) {
    history.pushState(null, '', path);
    loadTabContent(path);
}

// Initialize on DOM ready
function initTabs() {
    // Tab navigation using click
    document.querySelectorAll('.tab').forEach(tab => {
        tab.addEventListener('click', (e) => {
            e.preventDefault();
            const tabId = tab.dataset.tab;
            const href = TAB_PATHS[tabId];
            if (href) {
                navigateTo(href);
            }
        });
    });

    // Handle browser back/forward
    window.addEventListener('popstate', () => {
        loadTabContent(window.location.pathname);
    });
}

// Full width toggle
function toggleWidth() {
    const container = document.querySelector('.main-container');
    const btn = document.getElementById('width-toggle-btn');
    const isFull = container.classList.toggle('full-width');
    btn.textContent = isFull ? 'Full' : 'Normal';
    localStorage.setItem('fullWidth', isFull);
}

function initWidth() {
    const fullWidth = localStorage.getItem('fullWidth') === 'true';
    if (fullWidth) {
        const container = document.querySelector('.main-container');
        if (container) container.classList.add('full-width');
        const btn = document.getElementById('width-toggle-btn');
        if (btn) btn.textContent = 'Full';
    }
}

// Initialize width on load
initWidth();

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        initTabs();
        initViewSwitcher();
        loadTabContent(window.location.pathname);
    });
} else {
    initTabs();
    initViewSwitcher();
    loadTabContent(window.location.pathname);
}

let benchmarkView = localStorage.getItem('benchmarkView') || 'grid';

function initViewSwitcher() {
    const gridBtn = document.getElementById('view-grid');
    const listBtn = document.getElementById('view-list');
    const resultsContainer = document.getElementById('benchmark-results');

    if (!gridBtn || !listBtn || !resultsContainer) return;

    // Set initial state
    if (benchmarkView === 'list') {
        gridBtn.classList.remove('active');
        listBtn.classList.add('active');
        resultsContainer.classList.remove('results-grid');
        resultsContainer.classList.add('results-list');
    }

    gridBtn.addEventListener('click', () => {
        benchmarkView = 'grid';
        localStorage.setItem('benchmarkView', 'grid');
        gridBtn.classList.add('active');
        listBtn.classList.remove('active');
        resultsContainer.classList.add('results-grid');
        resultsContainer.classList.remove('results-list');
        loadBenchmarks();
    loadLibraries();
    });

    listBtn.addEventListener('click', () => {
        benchmarkView = 'list';
        localStorage.setItem('benchmarkView', 'list');
        listBtn.classList.add('active');
        gridBtn.classList.remove('active');
        resultsContainer.classList.add('results-list');
        resultsContainer.classList.remove('results-grid');
        loadBenchmarks();
    loadLibraries();
    });
}

async function searchHF() {
    const query = document.getElementById('hf-search').value.trim();
    if (!query) return;

    const resultsDiv = document.getElementById('search-results');
    resultsDiv.innerHTML = '<p>Searching...</p>';

    try {
        const response = await fetch(`/api/models/search?q=${encodeURIComponent(query)}`);
        const models = await response.json();

        if (models.length === 0) {
            resultsDiv.innerHTML = '<p>No models found. Try a different search term.</p>';
            return;
        }

        resultsDiv.innerHTML = models.map(m => `
            <div class="model-result" onclick="showDownloadModal('${m.id}')">
                <h4>${m.id}</h4>
                <p>${(m.downloads || 0).toLocaleString()} downloads</p>
                <p>❤️ ${m.likes || 0} likes</p>
            </div>
        `).join('');
    } catch (error) {
        resultsDiv.innerHTML = '<p>Error searching models. Please try again.</p>';
        console.error(error);
    }
}

async function showDownloadModal(modelId) {
    console.log('Opening download modal for:', modelId);
    selectedModelId = modelId;
    selectedFiles = [];

    const modal = document.getElementById('download-modal');
    document.getElementById('modal-model-id').textContent = modelId;
    document.getElementById('modal-files').innerHTML = '<p>Loading files...</p>';
    modal.classList.add('active');

    try {
        console.log('Fetching files from API:', `/api/models/files/${encodeURIComponent(modelId)}`);
        const response = await fetch(`/api/models/files/${encodeURIComponent(modelId)}`);
        console.log('API response status:', response.status);

        const data = await response.json();
        console.log('API response data:', data);

        if (data.files.length === 0) {
            console.log('No files found, checking raw HF API...');
            // Debug: show raw HF response
            const hfUrl = `https://huggingface.co/api/models/${modelId}/tree/main`;
            console.log('HF API URL:', hfUrl);

            document.getElementById('modal-files').innerHTML = `
                <p style="color: #fbbf24;">No GGUF files found automatically.</p>
                <p style="font-size: 0.85rem; color: var(--text-secondary); margin: 12px 0;">
                    This model may have files in subdirectories. Try downloading directly:
                </p>
                <div style="display: flex; gap: 8px; margin-top: 16px;">
                    <input type="text" 
                           id="manual-filename" 
                           placeholder="e.g., Q4_K_M/model.gguf or modelname-Q4_K_M.gguf"
                           style="flex: 1; padding: 10px; background: var(--bg-primary); border: 1px solid var(--border); border-radius: 6px; color: var(--text-primary);">
                    <button onclick="downloadManual()" class="btn primary">Download</button>
                </div>
                <p style="font-size: 0.8rem; color: var(--text-muted); margin-top: 8px;">
                    Check file structure at: <a href="https://huggingface.co/${modelId}/tree/main" target="_blank" style="color: var(--accent);">huggingface.co/${modelId}</a>
                </p>
            `;
            return;
        }

        const recommended = data.recommended || [];
        console.log('Recommended quantizations:', recommended);

        document.getElementById('modal-files').innerHTML = data.files.map(f => `
            <div class="file-item">
                <label>
                    <input type="checkbox"
                           value="${f.name}"
                           data-size="${f.size}"
                           data-quant="${f.quantization}"
                           onchange="toggleFile(this)"
                           ${recommended.includes(f.quantization) ? 'checked' : ''}>
                    <span class="file-name">${f.name}</span>
                </label>
                <span class="file-info">${f.size_human} · ${f.quantization}</span>
            </div>
        `).join('');
    } catch (error) {
        console.error('Error loading files:', error);
        document.getElementById('modal-files').innerHTML = `<p style="color: #ef4444;">Error loading files: ${error.message}</p>`;
    }
}

async function loadSafetensorsFiles(modelId) {
    try {
        const response = await fetch(`/api/models/all-files/${encodeURIComponent(modelId)}`);
        const data = await response.json();

        if (data.files.length === 0) {
            document.getElementById('modal-files').innerHTML = '<p>No compatible files found for this model.</p>';
            return;
        }

        const safetensorsFiles = data.files.filter(f => f.type === 'safetensors' || f.type === 'bin');

        if (safetensorsFiles.length === 0) {
            document.getElementById('modal-files').innerHTML = '<p>Model files found but not in compatible format.</p>';
            return;
        }

        document.getElementById('modal-files').innerHTML = `
            <div style="margin-bottom: 15px; padding: 10px; background: #e3f2fd; border-radius: 5px;">
                <p><strong>This model doesn't have GGUF files.</strong></p>
                <p>Download the safetensors model and convert it to GGUF.</p>
            </div>
            <div class="file-item">
                <label>
                    <input type="checkbox" checked disabled>
                    <span>${safetensorsFiles.length} safetensors files</span>
                </label>
                <span class="file-info">${safetensorsFiles.map(f => f.size_human).join(' + ')}</span>
            </div>
            <button onclick="downloadAndConvert('${modelId}')" class="btn primary" style="width: 100%; margin-top: 10px;">
                Download & Convert to GGUF
            </button>
        `;
    } catch (error) {
        document.getElementById('modal-files').innerHTML = '<p>Error loading model files.</p>';
        console.error(error);
    }
}

async function downloadAndConvert(modelId) {
    const filesDiv = document.getElementById('modal-files');
    filesDiv.innerHTML = '<p>Downloading model files... This may take several minutes.</p>';

    try {
        const response = await fetch('/api/models/download-all', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ model_id: modelId })
        });
        const result = await response.json();

        if (!result.success && result.errors && result.errors.length > 0) {
            filesDiv.innerHTML = '<p>Download completed with errors. Attempting conversion...</p>';
        }

        if (result.downloaded && result.downloaded.length > 0) {
            filesDiv.innerHTML = '<p>Download complete. Starting conversion to GGUF...</p>';

            const convertResponse = await fetch('/api/models/convert', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    model_id: modelId,
                    out_type: 'Q4_K_M'
                })
            });
            const convertResult = await convertResponse.json();

            if (convertResult.success) {
                filesDiv.innerHTML = `
                    <p style="color: green;"><strong>Conversion successful!</strong></p>
                    <p>GGUF files created: ${convertResult.files.join(', ')}</p>
                    <button onclick="location.reload()" class="btn primary" style="width: 100%; margin-top: 10px;">
                        Refresh to use model
                    </button>
                `;
            } else {
                filesDiv.innerHTML = `
                    <p style="color: red;"><strong>Conversion failed:</strong></p>
                    <p>${convertResult.error || 'Unknown error'}</p>
                    <p>Make sure convert-hf-to-gguf.py is available.</p>
                `;
            }
        } else {
            filesDiv.innerHTML = '<p>Download failed. Please try again.</p>';
        }
    } catch (error) {
        filesDiv.innerHTML = '<p>Error: ' + error.message + '</p>';
        console.error(error);
    }
}

function toggleFile(checkbox) {
    const filename = checkbox.value;
    if (checkbox.checked) {
        if (!selectedFiles.includes(filename)) {
            selectedFiles.push(filename);
        }
    } else {
        selectedFiles = selectedFiles.filter(f => f !== filename);
    }
}

function closeModal(modalId) {
    document.getElementById(modalId).classList.remove('active');
}

/**
 * Show a custom confirmation dialog instead of browser confirm()
 */
function showConfirm(title, message, onConfirm) {
    const modal = document.getElementById('dialog-modal');
    document.getElementById('dialog-title').textContent = title;
    document.getElementById('dialog-message').textContent = message;

    const confirmBtn = document.getElementById('dialog-confirm');
    const cancelBtn = document.getElementById('dialog-cancel');

    // Recreate buttons to clear old event listeners
    const newConfirm = confirmBtn.cloneNode(true);
    const newCancel = cancelBtn.cloneNode(true);
    confirmBtn.parentNode.replaceChild(newConfirm, confirmBtn);
    cancelBtn.parentNode.replaceChild(newCancel, cancelBtn);

    newConfirm.addEventListener('click', () => {
        closeModal('dialog-modal');
        if (onConfirm) onConfirm();
    });

    newCancel.addEventListener('click', () => {
        closeModal('dialog-modal');
    });

    modal.classList.add('active');
}

/**
 * Show a custom prompt dialog instead of browser prompt()
 */
function showPrompt(title, message, defaultValue, onConfirm) {
    const modal = document.getElementById('dialog-modal');
    document.getElementById('dialog-title').textContent = title;
    document.getElementById('dialog-message').textContent = message;

    const inputContainer = document.getElementById('dialog-input-container');
    const input = document.getElementById('dialog-input');
    inputContainer.style.display = 'block';
    input.value = defaultValue || '';

    const confirmBtn = document.getElementById('dialog-confirm');
    const cancelBtn = document.getElementById('dialog-cancel');

    // Recreate buttons
    const newConfirm = confirmBtn.cloneNode(true);
    const newCancel = cancelBtn.cloneNode(true);
    confirmBtn.parentNode.replaceChild(newConfirm, confirmBtn);
    cancelBtn.parentNode.replaceChild(newCancel, cancelBtn);

    newConfirm.addEventListener('click', () => {
        const value = input.value;
        closeModal('dialog-modal');
        inputContainer.style.display = 'none';
        if (onConfirm) onConfirm(value);
    });

    newCancel.addEventListener('click', () => {
        closeModal('dialog-modal');
        inputContainer.style.display = 'none';
    });

    modal.classList.add('active');
    setTimeout(() => input.focus(), 100);
}

async function downloadManual() {
    const filename = document.getElementById('manual-filename').value.trim();
    if (!filename) {
        showNotification('Please enter a filename', 'error');
        return;
    }

    console.log('Manual download:', selectedModelId, filename);

    const filesDiv = document.getElementById('modal-files');
    filesDiv.innerHTML = '<p>Downloading...</p>';

    try {
        const response = await fetch('/api/models/download', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                model_id: selectedModelId,
                filename: filename
            })
        });

        const result = await response.json();
        console.log('Download result:', result);

        if (result.success) {
            filesDiv.innerHTML = `
                <p style="color: #22c55e;"><strong>Download successful!</strong></p>
                <p>Saved to: ${result.path}</p>
                <p>Size: ${(result.size / 1024 / 1024).toFixed(2)} MB</p>
                <button onclick="location.reload()" class="btn primary" style="width: 100%; margin-top: 16px;">
                    Refresh to use model
                </button>
            `;
        } else {
            filesDiv.innerHTML = `<p style="color: #ef4444;">Download failed: ${result.error}</p>`;
        }
    } catch (error) {
        console.error('Download error:', error);
        filesDiv.innerHTML = `<p style="color: #ef4444;">Error: ${error.message}</p>`;
    }
}

async function downloadSelected() {
    if (selectedFiles.length === 0) {
        showNotification('Please select at least one file to download.', 'error');
        return;
    }

    console.log('Downloading files:', selectedFiles);
    const filesDiv = document.getElementById('modal-files');
    filesDiv.innerHTML = '<p>Starting downloads...</p>';

    let successCount = 0;
    let errors = [];
    let downloadIds = [];

    for (const filename of selectedFiles) {
        try {
            console.log('Starting download:', selectedModelId, filename);
            const response = await fetch('/api/models/download', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    model_id: selectedModelId,
                    filename: filename
                })
            });

            const result = await response.json();
            console.log('Download result:', result);

            if (result.success) {
                successCount++;
                downloadIds.push(result.download_id);
            } else {
                errors.push(`${filename}: ${result.error}`);
            }
        } catch (error) {
            console.error(`Error downloading ${filename}:`, error);
            errors.push(`${filename}: ${error.message}`);
        }
    }

    closeModal('download-modal');

    if (errors.length > 0) {
        showNotification(`Started ${successCount}/${selectedFiles.length} downloads. Some failed: ${errors.join(' | ')}`, 'error', 8000);
    } else {
        showNotification(`Started ${successCount} download(s). Check progress in Models tab.`, 'success');
    }

    if (successCount > 0) {
        loadDownloads();
        navigateTo('/models');
    }
}

async function loadDownloads() {
    try {
        const response = await fetch('/api/downloads');
        const downloads = await response.json();

        const modelsResponse = await fetch('/api/models');
        const models = await modelsResponse.json();

        const groups = {};

        // Helper to add to group
        const addToGroup = (groupName, item) => {
            if (!groups[groupName]) {
                groups[groupName] = [];
            }
            groups[groupName].push(item);
        };

        // Add downloading items
        downloads.forEach(d => {
            const groupName = d.repo_id.replace('/', '_');
            addToGroup(groupName, {
                type: 'download',
                id: d.id,
                name: d.filename,
                repo_id: d.repo_id,
                status: d.status,
                progress: d.progress,
                downloaded_bytes: d.downloaded_bytes,
                total_bytes: d.total_bytes,
                speed: d.speed_bytes_per_sec,
                eta: d.eta_seconds,
                error: d.error
            });
        });

        // Add completed model items
        models.forEach(m => {
            const groupName = m.name;
            // Check if model item already exists in the group
            const exists = groups[groupName]?.some(i => i.type === 'model');
            if (!exists) {
                addToGroup(groupName, {
                    type: 'model',
                    name: m.name,
                    status: 'completed'
                });
            }
        });

        renderDownloadsAndModels(groups);
    } catch (error) {
        console.error('Error loading downloads:', error);
    }
}

function toggleGroup(groupName) {
    if (expandedGroups.has(groupName)) {
        expandedGroups.delete(groupName);
    } else {
        expandedGroups.add(groupName);
    }
    const container = document.getElementById('downloads-list');
    if (container) {
        const groupEl = container.querySelector(`.download-group[data-group="${groupName}"]`);
        if (groupEl) groupEl.classList.toggle('expanded');
    }
}

function renderDownloadsAndModels(groups) {
    const container = document.getElementById('downloads-list');
    if (!container) return;

    const groupNames = Object.keys(groups);
    if (groupNames.length === 0) {
        container.innerHTML = `
            <div class="empty-state">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                    <polyline points="7 10 12 15 17 10"/>
                    <line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                <p>No downloads or models yet.</p>
                <p style="color: var(--text-muted);">Search for a model to download.</p>
            </div>
        `;
        return;
    }

    container.innerHTML = groupNames.map(groupName => {
        const items = groups[groupName];
        const isExpanded = expandedGroups.has(groupName);

        const activeDownloads = items.filter(i => i.type === 'download' && ['downloading', 'pending', 'queued'].includes(i.status)).length;
        const failedDownloads = items.filter(i => i.type === 'download' && i.status.includes('failed')).length;

        let statusBadge = '';
        if (activeDownloads > 0) {
            statusBadge = `<span class="badge running">Active (${activeDownloads})</span>`;
        } else if (failedDownloads > 0) {
            statusBadge = `<span class="badge failed">Failed (${failedDownloads})</span>`;
        } else if (items.every(i => i.status === 'completed')) {
            statusBadge = `<span class="badge running">Completed</span>`;
        }

        return `
            <div class="download-group ${isExpanded ? 'expanded' : ''}" data-group="${groupName}">
                <div class="download-group-header" onclick="toggleGroup('${groupName}')">
                    <div class="download-group-title">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width: 18px; height: 18px; color: var(--accent);">
                            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                        </svg>
                        <span>${groupName}</span>
                        ${statusBadge}
                    </div>
                    <div class="download-group-stats">
                        <span>${items.length} file(s)</span>
                        <div class="download-group-toggle">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width: 16px; height: 16px;">
                                <polyline points="6 9 12 15 18 9"></polyline>
                            </svg>
                        </div>
                    </div>
                </div>
                <div class="download-group-content">
                    ${items.map(item => {
            if (item.type === 'download') {
                const statusColors = {
                    'pending': '#fbbf24',
                    'queued': '#fbbf24',
                    'downloading': '#6366f1',
                    'paused': '#f59e0b',
                    'completed': '#22c55e',
                    'failed': '#ef4444',
                    'failed_recovered': '#ef4444',
                    'cancelled': '#71717a'
                };
                const color = statusColors[item.status] || '#71717a';
                const progress = item.progress || 0;
                const downloadedMB = (item.downloaded_bytes / 1024 / 1024).toFixed(1);
                const totalMB = (item.total_bytes / 1024 / 1024).toFixed(1);
                const speedMB = ((item.speed || 0) / 1024 / 1024).toFixed(1);

                let statusText = 'Queued';
                if (item.status === 'downloading') {
                    statusText = `Downloading ${progress.toFixed(1)}%`;
                } else if (item.status === 'pending') {
                    statusText = 'Queued';
                } else if (item.status === 'completed') {
                    statusText = 'Completed';
                } else if (item.status === 'paused') {
                    statusText = `Paused at ${progress.toFixed(1)}%`;
                } else if (item.status === 'failed' || item.status === 'failed_recovered') {
                    statusText = `Failed: ${item.error || 'Unknown error'}`;
                } else if (item.status === 'cancelled') {
                    statusText = 'Cancelled';
                }

                const actionButtons = [];
                if (item.status === 'downloading') {
                    actionButtons.push(`<button onclick="pauseDownload('${item.id}')" class="btn small secondary">Pause</button>`);
                    actionButtons.push(`<button onclick="cancelDownload('${item.id}')" class="btn small danger">Cancel</button>`);
                } else if (item.status === 'paused') {
                    actionButtons.push(`<button onclick="resumeDownload('${item.id}')" class="btn small">Resume</button>`);
                    actionButtons.push(`<button onclick="restartDownload('${item.id}')" class="btn small secondary">Restart</button>`);
                    actionButtons.push(`<button onclick="cancelDownload('${item.id}')" class="btn small danger">Cancel</button>`);
                } else if (item.status === 'failed' || item.status === 'failed_recovered' || item.status === 'cancelled') {
                    actionButtons.push(`<button onclick="resumeDownload('${item.id}')" class="btn small">Resume</button>`);
                    actionButtons.push(`<button onclick="restartDownload('${item.id}')" class="btn small secondary">Restart</button>`);
                    actionButtons.push(`<button onclick="deleteDownload('${item.id}')" class="btn small danger">Delete</button>`);
                } else if (item.status === 'completed') {
                    actionButtons.push(`<button onclick="startWithModel('${item.name}')" class="btn small">Start</button>`);
                    actionButtons.push(`<button onclick="restartDownload('${item.id}')" class="btn small secondary">Redownload</button>`);
                    actionButtons.push(`<button onclick="deleteDownload('${item.id}')" class="btn small danger">Delete</button>`);
                } else {
                    actionButtons.push(`<button onclick="pauseDownload('${item.id}')" class="btn small secondary">Pause</button>`);
                    actionButtons.push(`<button onclick="cancelDownload('${item.id}')" class="btn small danger">Cancel</button>`);
                }

                return `
                                <div class="download-item" data-id="${item.id}" onclick="event.stopPropagation()">
                                    <div class="download-info">
                                        <div class="download-name">${item.name}</div>
                                        <div class="download-status" style="color: ${color};">
                                            ${statusText}
                                        </div>
                                        ${['downloading', 'paused'].includes(item.status) ? `
                                            <div class="download-progress-bar">
                                                <div class="download-progress-fill" style="width: ${progress}%;"></div>
                                            </div>
                                            <div class="download-details">
                                                ${downloadedMB} / ${totalMB} MB · ${speedMB} MB/s
                                            </div>
                                        ` : ''}
                                    </div>
                                    <div class="download-actions">
                                        ${actionButtons.join('')}
                                    </div>
                                </div>
                            `;
            } else {
                return `
                                <div class="model-item" onclick="event.stopPropagation()">
                                    <div class="model-name">
                                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width: 14px; height: 14px; margin-right: 6px; color: var(--success);">
                                            <polyline points="20 6 9 17 4 12"></polyline>
                                        </svg>
                                        ${item.name}
                                    </div>
                                    <div class="model-actions">
                                        <button onclick="startWithModel('${item.name}')" class="btn small">Start</button>
                                    </div>
                                </div>
                            `;
            }
        }).join('')}
                </div>
            </div>
        `;
    }).join('');
}

async function deleteDownload(downloadId) {
    showConfirm('Delete Download', 'Are you sure you want to delete this download?', async () => {
        try {
            const response = await fetch(`/api/downloads/${downloadId}`, { method: 'DELETE' });
            if (response.ok) {
                loadDownloads();
                showNotification('Download deleted.', 'success');
            }
        } catch (error) {
            console.error('Error deleting download:', error);
            showNotification('Failed to delete download.', 'error');
        }
    });
}

async function retryDownload(downloadId) {
    return resumeDownload(downloadId);
}

async function pauseDownload(downloadId) {
    try {
        const response = await fetch(`/api/downloads/${downloadId}/pause`, { method: 'POST' });
        const result = await response.json();
        if (response.ok && result.success) {
            showNotification('Download paused.', 'info');
        } else {
            showNotification(result.error || 'Unable to pause download.', 'error');
        }
    } catch (error) {
        showNotification('Error pausing download.', 'error');
    }
    loadDownloads();
}

async function resumeDownload(downloadId) {
    try {
        const response = await fetch(`/api/downloads/${downloadId}/resume`, { method: 'POST' });
        const result = await response.json();
        if (response.ok && result.success) {
            showNotification('Download resumed.', 'success');
        } else {
            showNotification(result.error || 'Unable to resume download.', 'error');
        }
    } catch (error) {
        showNotification('Error resuming download.', 'error');
    }
    loadDownloads();
}

async function restartDownload(downloadId) {
    try {
        const response = await fetch(`/api/downloads/${downloadId}/restart`, { method: 'POST' });
        const result = await response.json();
        if (response.ok && result.success) {
            showNotification('Download restarted from beginning.', 'success');
        } else {
            showNotification(result.error || 'Unable to restart download.', 'error');
        }
    } catch (error) {
        showNotification('Error restarting download.', 'error');
    }
    loadDownloads();
}

async function cancelDownload(downloadId) {
    try {
        const response = await fetch(`/api/downloads/${downloadId}/cancel`, { method: 'POST' });
        const result = await response.json();
        if (response.ok && result.success) {
            showNotification('Download cancelled.', 'info');
        } else {
            showNotification(result.error || 'Unable to cancel download.', 'error');
        }
    } catch (error) {
        showNotification('Error cancelling download.', 'error');
    }
    loadDownloads();
}

async function deleteModel(modelName) {
    showConfirm('Delete Model', `Delete model "${modelName}"? This will remove all files.`, async () => {
        try {
            const response = await fetch(`/api/models/${modelName}`, { method: 'DELETE' });
            const result = await response.json();

            if (result.success) {
                showNotification('Model deleted successfully.', 'success');
                loadDownloads();
                loadModels();
            } else {
                showNotification('Failed to delete model: ' + (result.error || 'Unknown error'), 'error');
            }
        } catch (error) {
            showNotification('Error deleting model: ' + error.message, 'error');
        }
    });
}

// Auto-refresh downloads every 2 seconds when on models tab
setInterval(() => {
    const modelsTab = document.getElementById('models');
    if (modelsTab && modelsTab.classList.contains('active')) {
        loadDownloads();
    }
}, 2000);

function updateQuantizationSelect(modelSelect, quantSelectId) {
    if (!modelSelect || modelSelect.selectedIndex === -1) return;

    const selectedOption = modelSelect.options[modelSelect.selectedIndex];
    if (!selectedOption) return;

    const quants = selectedOption.getAttribute('data-quants');
    const quantSelect = document.getElementById(quantSelectId);

    if (!quants || !quantSelect) {
        if (quantSelect) quantSelect.innerHTML = '<option value="">Default</option>';
        return;
    }

    const quantList = quants.split(',');
    const currentValue = quantSelect.value;

    quantSelect.innerHTML = '<option value="">Default</option>';
    quantList.forEach(q => {
        if (q.trim()) {
            const option = document.createElement('option');
            option.value = q.trim();
            option.textContent = q.trim();
            quantSelect.appendChild(option);
        }
    });

    if (currentValue && quantList.includes(currentValue)) {
        quantSelect.value = currentValue;
    }
}

function getModelPath(modelName, quantization) {
    if (quantization && quantization.trim()) {
        return `${modelsBasePath}/${modelName}/${modelName.split('/').pop().replace('_', '-')}-${quantization}.gguf`;
    }
    return `${modelsBasePath}/${modelName}`;
}

async function startWithModel(modelName) {
    showPrompt('Start Server', 'Enter port for server:', '8080', async (port) => {
        if (!port) return;

        try {
            const response = await fetch('/api/servers', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    backend: 'upstream',
                    model_path: getModelPath(modelName, ''),
                    model: modelName,
                    port: parseInt(port)
                })
            });

            const result = await response.json();
            if (result.success) {
                showNotification(`Server started! Access at http://localhost:${port}`, 'success');
            } else {
                showNotification('Failed to start server: ' + (result.error || 'Unknown error'), 'error');
            }
        } catch (error) {
            showNotification('Error starting server: ' + error.message, 'error');
        }
    });
}

async function runBenchmark(e) {
    if (e) e.preventDefault();

    const formData = new FormData(document.getElementById('benchmark-form'));
    const config = Object.fromEntries(formData);

    const modelName = config.model;
    const quantization = config.quantization;
    const modelPath = getModelPath(modelName, quantization);

    try {
        const response = await fetch('/api/benchmarks', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                backend_type: config.backend_type,
                model_path: modelPath,
                model: modelName,
                vendor_backend: config.vendor_backend,
                ctx_size: parseInt(config.ctx_size),
                n_gpu_layers: parseInt(config.n_gpu_layers),
                n_threads: parseInt(config.n_threads),
                prompt_tokens: parseInt(config.prompt_tokens),
                generated_tokens: parseInt(config.generated_tokens)
            })
        });

        const result = await response.json();
        if (result.success) {
            showNotification('Benchmark started! Check results in a few moments.', 'success');
        } else {
            showNotification('Failed to start benchmark: ' + (result.error || 'Unknown error'), 'error');
        }
    } catch (error) {
        showNotification('Error running benchmark: ' + error.message, 'error');
    }
}

async function stopServer(serverId) {
    showConfirm('Stop Server', 'Are you sure you want to stop this server?', async () => {
        try {
            const response = await fetch(`/api/servers/${serverId}/stop`, { method: 'POST' });
            const result = await response.json();

            if (result.success) {
                showNotification('Server stopped successfully.', 'success');
                loadServers();
                loadDashboard();
            } else {
                showNotification('Failed to stop server: ' + (result.error || 'Unknown error'), 'error');
            }
        } catch (error) {
            console.error('Error stopping server:', error);
            showNotification('Error stopping server: ' + error.message, 'error');
        }
    });
}

// Stress test management
async function startStressTest(serverId) {
    document.getElementById('stress-modal').classList.add('active');
    document.getElementById('stress-form').setAttribute('data-server-id', serverId);
}

async function stopStressTest(serverId) {
    showConfirm('Stop Stress Test', 'Stop the stress test for this server?', async () => {
        try {
            const response = await fetch(`/api/servers/${serverId}/stress-test`, { method: 'DELETE' });
            const result = await response.json();

            if (response.ok) {
                showNotification('Stress test stopped.', 'success');
                updateStressTestStatus(serverId);
            } else {
                showNotification('Failed to stop stress test.', 'error');
            }
        } catch (error) {
            showNotification('Error stopping stress test.', 'error');
        }
    });
}

async function updateStressTestStatus(serverId) {
    try {
        const response = await fetch(`/api/servers/${serverId}/stress-test`);
        const status = await response.json();

        const button = document.getElementById(`stress-btn-${serverId}`);
        if (button) {
            if (status.running) {
                button.textContent = 'Stop Stress Test';
                button.className = 'btn small danger';
                button.onclick = () => stopStressTest(serverId);
            } else {
                button.textContent = 'Stress Test';
                button.className = 'btn small';
                button.onclick = () => startStressTest(serverId);
            }
        }
    } catch (error) {
        console.error('Error checking stress test status:', error);
    }
}

document.getElementById('hf-search').addEventListener('keypress', (e) => {
    if (e.key === 'Enter') searchHF();
});

document.querySelectorAll('input[type="range"]').forEach(range => {
    const display = range.parentElement.querySelector('.value-display');
    if (display) {
        display.textContent = range.value;
        range.addEventListener('input', () => {
            display.textContent = range.value;
        });
    }
});

document.getElementById('settings-form').addEventListener('submit', (e) => {
    e.preventDefault();
    showNotification('Settings saved!', 'success');
});

document.getElementById('advanced-form').addEventListener('submit', (e) => {
    e.preventDefault();
    showNotification('Advanced settings saved!', 'success');
});

// Stress test form handler
document.getElementById('stress-form').addEventListener('submit', async (e) => {
    e.preventDefault();

    const formData = new FormData(e.target);
    const config = Object.fromEntries(formData);
    const serverId = e.target.getAttribute('data-server-id');

    try {
        const response = await fetch(`/api/servers/${serverId}/stress-test`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                prompt: config.prompt,
                max_tokens: parseInt(config.max_tokens),
                delay: parseFloat(config.delay)
            })
        });

        const result = await response.json();
        if (response.ok) {
            showNotification('Stress test started!', 'success');
            closeModal('stress-modal');
            updateStressTestStatus(serverId);
        } else {
            showNotification('Failed to start stress test.', 'error');
        }
    } catch (error) {
        showNotification('Error starting stress test.', 'error');
    }
});

// Benchmark functionality
async function loadBenchmarks() {
    try {
        const response = await fetch('/api/benchmarks');
        const results = await response.json();
        window.allBenchmarks = results;
        populateScalingSelect(results);
        const container = document.getElementById('benchmark-results');

        if (results.length === 0) {
            container.innerHTML = '<p>No benchmarks run yet. Run a benchmark to see results here.</p>';
            return;
        }

        let html = '';

        if (benchmarkView === 'list') {
            html += `
                <div class="model-result header" style="cursor: default; background: transparent; border: none; font-weight: 600; color: var(--text-muted); font-size: 0.8rem; text-transform: uppercase;">
                    <div>Model & Backend</div>
                    <div>Perf</div>
                    <div>VRAM</div>
                    <div>Time</div>
                    <div>Status</div>
                    <div>Actions</div>
                </div>
            `;
        }

        html += results.map(r => `
            <div class="model-result" onclick="showBenchmarkDetail('${r.id}')">
                <h4>${r.model} <small style="color: var(--text-muted); font-weight: normal;">(${r.backend_type})</small></h4>
                <p>${r.tokens_per_second?.toFixed(2) || 'N/A'}<span style="font-size: 0.75rem; color: var(--text-muted); margin-left: 4px;">T/s</span></p>
                <p>${r.memory_gpu_vram_mb?.toFixed(0) || 0}<span style="font-size: 0.75rem; color: var(--text-muted); margin-left: 4px;">MB</span></p>
                <p style="font-size: 0.8rem;">${new Date(r.timestamp).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}</p>
                <span class="badge ${r.status}">${r.status}</span>
                <div class="result-actions" onclick="event.stopPropagation()">
                    <button onclick="exportBenchmark('${r.id}')" class="btn small" title="Export CSV">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width: 14px; height: 14px;"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                    </button>
                    <button onclick="deleteBenchmark('${r.id}')" class="btn small danger" title="Delete">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width: 14px; height: 14px;"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                    </button>
                </div>
                ${r.error && benchmarkView === 'grid' ? `<p class="error-text">Error: ${r.error.substring(0, 100)}${r.error.length > 100 ? '...' : ''}</p>` : ''}
            </div>
        `).join('');

        container.innerHTML = html;
    } catch (error) {
        console.error('Error loading benchmarks:', error);
    }
}

async function runBenchmark(e) {
    if (e) e.preventDefault();

    const formData = new FormData(document.getElementById('benchmark-form'));
    const config = Object.fromEntries(formData);

    const modelName = config.model;
    const modelPath = getModelPath(modelName, config.quantization || '');

    try {
        const response = await fetch('/api/benchmarks', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                backend_type: config.backend_type,
                model_path: modelPath,
                model: modelName,
                vendor_backend: config.vendor_backend,
                ctx_size: parseInt(config.ctx_size),
                n_gpu_layers: parseInt(config.n_gpu_layers),
                n_threads: parseInt(config.n_threads),
                prompt_tokens: parseInt(config.prompt_tokens),
                generated_tokens: parseInt(config.generated_tokens)
            })
        });

        const result = await response.json();
        if (result.success) {
            showNotification('Benchmark started! Running in background.', 'success');
            loadBenchmarks();
    loadLibraries();
        } else {
            showNotification('Failed to start benchmark: ' + (result.error || 'Unknown error'), 'error');
        }
    } catch (error) {
        showNotification('Error running benchmark: ' + error.message, 'error');
    }
}

async function runComparison() {
    const modelSelect = document.getElementById('bench-model');
    const modelName = modelSelect.value;
    const modelPath = getModelPath(modelName, '');

    try {
        const response = await fetch('/api/benchmarks/compare', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                model_path: modelPath,
                model: modelName
            })
        });

        const result = await response.json();
        showNotification('Comparison benchmark started! Results will appear below.', 'success');
        loadBenchmarks();
    loadLibraries();
    } catch (error) {
        showNotification('Error running comparison: ' + error.message, 'error');
    }
}

async function deleteBenchmark(benchId) {
    showConfirm('Delete Result', 'Delete this benchmark result?', async () => {
        try {
            const response = await fetch(`/api/benchmarks/${benchId}`, { method: 'DELETE' });
            if (response.ok) {
                loadBenchmarks();
    loadLibraries();
                showNotification('Benchmark result deleted.', 'success');
            }
        } catch (error) {
            console.error('Error deleting benchmark:', error);
            showNotification('Failed to delete result.', 'error');
        }
    });
}

function exportBenchmark(benchId) {
    window.open(`/api/benchmarks/export/${benchId}`, '_blank');
}

function showBenchmarkModal(modelName) {
    document.getElementById('bench-model').value = modelName;
    navigateTo('/benchmarks');
}

async function showBenchmarkDetail(benchId) {
    try {
        const response = await fetch(`/api/benchmarks/${benchId}`);
        const result = await response.json();

        const modal = document.createElement('div');
        modal.className = 'modal-overlay';
        modal.onclick = (e) => { if (e.target === modal) modal.remove(); };

        const formatBytes = (bytes) => {
            if (bytes === 0) return '0 B';
            const k = 1024;
            const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
            const i = Math.floor(Math.log(bytes) / Math.log(k));
            return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
        };

        const formatMs = (ms) => {
            if (!ms || ms === 0) return 'N/A';
            return ms.toFixed(2) + ' ms';
        };

        const details = [
            ['Status', `<span class="badge ${result.status}">${result.status}</span>`],
            ['Model', result.model],
            ['Backend', result.backend_type],
            ['Vendor', result.vendor_backend || 'auto'],
            ['Context Size', result.ctx_size?.toLocaleString() || 'N/A'],
            ['GPU Layers', result.n_gpu_layers?.toLocaleString() || '0'],
            ['Threads', result.n_threads?.toLocaleString() || 'N/A'],
            ['Model Size', formatBytes(result.model_size_mb * 1024 * 1024)],
            ['Quantization', result.model_quantization || 'Unknown'],
            ['Tokens/s', result.tokens_per_second?.toFixed(2) || 'N/A'],
            ['Batch Tokens/s', result.tokens_per_second_batch?.toFixed(2) || 'N/A'],
            ['Time to First Token', formatMs(result.time_to_first_token_ms)],
            ['Total Tokens', result.total_tokens?.toLocaleString() || 'N/A'],
            ['Generated Tokens', result.generated_tokens?.toLocaleString() || 'N/A'],
            ['GPU VRAM', result.memory_gpu_vram_mb ? result.memory_gpu_vram_mb.toFixed(0) + ' MB' : 'N/A'],
            ['CPU RAM', result.memory_cpu_ram_mb ? result.memory_cpu_ram_mb.toFixed(0) + ' MB' : 'N/A'],
            ['Timestamp', new Date(result.timestamp).toLocaleString()],
            ['Test Type', result.test_type || 'N/A'],
        ];

        let errorSection = '';
        if (result.error) {
            errorSection = `
                <div class="detail-error">
                    <strong>Error:</strong>
                    <pre>${result.error}</pre>
                </div>
            `;
        }

        let benchmarkOutput = '';
        if (result.benchmark_output) {
            benchmarkOutput = `
                <div class="detail-output">
                    <strong>Benchmark Output:</strong>
                    <pre>${result.benchmark_output}</pre>
                </div>
            `;
        }

        modal.innerHTML = `
            <div class="modal-content benchmark-detail">
                <button class="modal-close" onclick="modal.remove()">×</button>
                <h3>Benchmark Details</h3>
                <div class="detail-grid">
                    ${details.map(([label, value]) => `
                        <div class="detail-item">
                            <span class="detail-label">${label}</span>
                            <span class="detail-value">${value}</span>
                        </div>
                    `).join('')}
                </div>
                ${errorSection}
                ${benchmarkOutput}
                <div class="modal-actions">
                    <button onclick="exportBenchmark('${benchId}')" class="btn secondary">Export CSV</button>
                    <button onclick="deleteBenchmark('${benchId}'); modal.remove();" class="btn danger">Delete</button>
                </div>
            </div>
        `;

        document.body.appendChild(modal);
    } catch (error) {
        showNotification('Error loading benchmark details: ' + error.message, 'error');
    }
}

// Add benchmark form listener
document.getElementById('benchmark-form')?.addEventListener('submit', runBenchmark);

// Notification system
function showNotification(message, type = 'info', duration = 5000) {
    const container = document.getElementById('notification-container');
    if (!container) return;

    const notification = document.createElement('div');
    notification.className = `notification ${type}`;

    const icons = {
        success: '✓',
        error: '✕',
        info: 'ℹ'
    };

    notification.innerHTML = `
        <span class="icon">${icons[type] || icons.info}</span>
        <span class="message">${message}</span>
        <button class="close-btn" onclick="this.parentElement.remove()">×</button>
    `;

    container.appendChild(notification);

    if (duration > 0) {
        setTimeout(() => {
            notification.style.animation = 'slideIn 0.3s ease reverse';
            setTimeout(() => notification.remove(), 300);
        }, duration);
    }
}

async function loadSystemStatus() {
    try {
        const response = await fetch('/api/system/status');
        if (!response.ok) return;

        const data = await response.json();
        if (data.models_dir) {
            modelsBasePath = data.models_dir;
        }

        const dbConnected = document.getElementById('db-connected');
        const dbPath = document.getElementById('db-path');
        const dbDownloads = document.getElementById('db-download-count');
        const dbBenchmarks = document.getElementById('db-benchmark-count');
        if (dbConnected) dbConnected.textContent = data.connected ? 'Connected' : 'Offline';
        if (dbPath) dbPath.textContent = data.db_path || '-';
        if (dbDownloads) dbDownloads.textContent = String(data.counts?.downloads ?? 0);
        if (dbBenchmarks) dbBenchmarks.textContent = String(data.counts?.benchmarks ?? 0);
    } catch (error) {
        console.error('Error loading system status:', error);
    }
}

async function loadEngineStatus() {
    try {
        const response = await fetch('/api/engines/llama/status');
        if (!response.ok) return;
        const data = await response.json();

        const statusEl = document.getElementById('engine-status');
        const modeEl = document.getElementById('engine-mode');
        const capsEl = document.getElementById('engine-capabilities');
        const errorEl = document.getElementById('engine-error');

        const caps = [];
        if (data.has_cpu) caps.push('CPU');
        if (data.has_cuda) caps.push('CUDA');
        if (data.has_vulkan) caps.push('Vulkan');

        const phase = data.details?.phase;
        const statusText = data.installing
            ? `installing${phase ? ` (${phase})` : ''}`
            : (data.status || 'unknown');

        if (statusEl) statusEl.textContent = statusText;
        if (modeEl) modeEl.textContent = data.install_mode || '-';
        if (capsEl) capsEl.textContent = caps.length ? caps.join(', ') : 'None';
        if (errorEl) errorEl.textContent = data.last_error || '-';
    } catch (error) {
        console.error('Error loading engine status:', error);
    }
}

async function loadEngines() {
    try {
        const response = await fetch('/api/engines');
        if (!response.ok) return;
        const engines = await response.json();

        const container = document.getElementById('engines-list');
        if (!container) return;

        if (engines.length === 0) {
            container.innerHTML = '<p class="empty-state">No engines configured</p>';
            return;
        }

        container.innerHTML = engines.map(engine => {
            const hasUpdate = engine.hasSource && engine.installedVersion !== engine.latestVersion && engine.latestVersion !== '-';
            
            const launchMethodsHtml = engine.launchMethods.length > 0
                ? engine.launchMethods.map(m => `
                    <div class="launch-method">
                        <span class="badge origin ${m.type}">${m.type}</span>
                        <span class="mono">${m.provider}</span>
                        <span class="backend-tags">${m.backends.map(b => `<span class="badge ${b}">${b}</span>`).join('')}</span>
                    </div>
                `).join('')
                : '<span class="text-muted" style="font-size: 0.8rem;">No launch methods available</span>';

            const updateBtn = hasUpdate
                ? `<button class="btn small" onclick="updateEngine('${engine.id}')">Update</button>`
                : '';

            return `
                <div class="engine-card">
                    <div class="engine-card-header">
                        <div>
                            <span class="engine-name">${engine.name}</span>
                            <p class="engine-desc">${engine.description}</p>
                        </div>
                        <div class="engine-actions">
                            ${updateBtn}
                            <a href="${engine.source}" target="_blank" class="btn ghost small">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
                                    <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6"/>
                                    <polyline points="15,3 21,3 21,9"/>
                                    <line x1="10" y1="14" x2="21" y2="3"/>
                                </svg>
                            </a>
                        </div>
                    </div>
                    <div class="engine-card-body">
                        <div class="engine-version-row">
                            <div class="version-info">
                                <span class="version-label">Installed</span>
                                <span class="version-value ${engine.hasSource ? '' : 'text-muted'}">${engine.installedVersion}</span>
                                ${engine.installedDate ? `<span class="version-date">${engine.installedDate}</span>` : ''}
                            </div>
                            <div class="version-info">
                                <span class="version-label">Latest</span>
                                <span class="version-value ${hasUpdate ? 'has-update' : ''}">${engine.latestVersion}</span>
                                ${engine.latestDate ? `<span class="version-date">${engine.latestDate}</span>` : ''}
                            </div>
                        </div>
                        <div class="engine-launch-methods">
                            <span class="launch-label">Launch Methods</span>
                            ${launchMethodsHtml}
                        </div>
                    </div>
                </div>
            `;
        }).join('');
    } catch (error) {
        console.error('Error loading engines:', error);
    }
}

async function updateEngine(engineId) {
    try {
        const response = await fetch(`/api/engines/${engineId}/update`, { method: 'POST' });
        const result = await response.json();
        
        if (result.success) {
            showNotification(`Engine updated to ${result.version}`, 'success');
            await loadEngines();
        } else {
            showNotification(`Update failed: ${result.error}`, 'error');
        }
    } catch (error) {
        showNotification(`Update failed: ${error.message}`, 'error');
    }
}

async function ensureEngine(mode) {
    const statusEl = document.getElementById('engine-status');
    if (statusEl) statusEl.textContent = 'working...';

    try {
        const response = await fetch('/api/engines/llama/ensure', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ mode, async: true })
        });
        const result = await response.json();

        if (!result.success) {
            showNotification(`Engine setup failed: ${result.last_error || result.error || 'unknown error'}`, 'error', 7000);
            await loadEngineStatus();
            await loadSystemStatus();
            return;
        }

        if (result.installing || result.started) {
            showNotification('Engine setup started. Status will update automatically.', 'info');

            for (let attempt = 0; attempt < 180; attempt++) {
                await new Promise((resolve) => setTimeout(resolve, 2000));

                let status = null;
                try {
                    const statusResp = await fetch('/api/engines/llama/status');
                    if (statusResp.ok) status = await statusResp.json();
                } catch (_) {
                    status = null;
                }

                await loadEngineStatus();
                await loadSystemStatus();

                if (!status || status.installing) {
                    continue;
                }

                if (status.status === 'ready') {
                    showNotification('llama.cpp engine is ready.', 'success');
                } else {
                    showNotification(`Engine setup failed: ${status.last_error || 'unknown error'}`, 'error', 7000);
                }
                return;
            }

            showNotification('Engine setup is still running. You can keep polling from status.', 'info', 6000);
        } else if (result.status === 'ready') {
            showNotification('llama.cpp engine is ready.', 'success');
        } else {
            showNotification(`Engine setup status: ${result.status || 'unknown'}`, 'info');
        }
    } catch (error) {
        showNotification(`Engine setup request failed: ${error.message}`, 'error');
    }

    await loadEngineStatus();
    await loadSystemStatus();
}

// Auto-refresh benchmarks every 2 seconds when on benchmarks tab
setInterval(() => {
    const benchmarksTab = document.getElementById('benchmarks');
    if (benchmarksTab && benchmarksTab.classList.contains('active')) {
        loadBenchmarks();
    loadLibraries();
    }
}, 2000);

// Load benchmarks on page load
// Content visibility - fade in when ready
let contentVisible = false;

function showContent() {
    if (contentVisible) return;
    contentVisible = true;
    document.querySelectorAll('.tab-content').forEach(el => {
        el.style.opacity = '0';
        el.style.display = '';
        setTimeout(() => {
            el.style.transition = 'opacity 0.3s';
            el.style.opacity = '1';
        }, 10);
    });
}

async function loadServers() {
    try {
        const response = await fetch('/api/servers');
        const data = await response.json();
        const servers = data.data || [];
        const container = document.getElementById('servers-list');
        
        if (container) {
            document.getElementById('dashboard-server-count').textContent = 
                `${servers.length} server(s) running`;
            
            if (servers.length === 0) {
                container.innerHTML = '<div class="empty-state"><p>No servers running.</p></div>';
            } else {
                container.innerHTML = servers.map(s => `
                    <div class="server-item" data-id="${s.id}">
                        <div class="server-info">
                            <span class="server-name">${s.model || 'Unknown'}</span>
                            <span class="badge ${s.status === 'running' ? 'success' : 'secondary'}">${s.status || 'unknown'}</span>
                            <span class="server-details">${s.backend || ''} ${s.ctx_size ? '@ ' + s.ctx_size + ' ctx' : ''}</span>
                        </div>
                        <div class="server-actions">
                            <button onclick="openChat('${s.id}')" class="btn small">Chat</button>
                            <button onclick="stopServer('${s.id}')" class="btn small danger">Stop</button>
                        </div>
                    </div>
                `).join('');
            }
        }
    } catch (error) {
        console.error('Error loading servers:', error);
    }
}

document.addEventListener('DOMContentLoaded', () => {
    // Show content immediately
    showContent();
    
    // Load all data in parallel
    loadDashboardData();
    loadBenchmarks();
    loadLibraries();
    loadSystemStatus();
    loadEngineStatus();
    loadEngines();
    loadDownloads();
    loadServers();
    
    // Initialize auth status
    initAuth();
});

// Load dashboard data (backends, models for selects)
async function loadDashboardData() {
    try {
        // Load data in parallel
        const [backendsRes, modelsRes] = await Promise.all([
            fetch('/api/backends'),
            fetch('/api/models')
        ]);
        
        const [backends, models] = await Promise.all([
            backendsRes.json(),
            modelsRes.json()
        ]);
        
        // Populate backend selects
        const qsBackend = document.getElementById('qs-backend');
        const benchBackend = document.getElementById('bench-backend');
        const dashboardBackends = document.getElementById('dashboard-backends');
        
        if (qsBackend) {
            qsBackend.innerHTML = backends.map(b => 
                `<option value="${b.type}">${b.name}</option>`
            ).join('');
        }
        
        if (benchBackend) {
            benchBackend.innerHTML = backends.map(b => 
                `<option value="${b.type}">${b.name}</option>`
            ).join('');
        }
        
        if (dashboardBackends) {
            if (backends.length === 0) {
                dashboardBackends.innerHTML = '<li class="empty-state">No engines found</li>';
            } else {
                dashboardBackends.innerHTML = backends.map(engine => {
                    const launchMethodsHtml = engine.launchMethods.length > 0
                        ? engine.launchMethods.map(m => `
                            <div class="launch-method">
                                <span class="badge origin ${m.type}">${m.type}</span>
                                <span class="backend-tags">${m.backends.map(b => `<span class="badge ${b}">${b}</span>`).join('')}</span>
                            </div>
                        `).join('')
                        : '<span class="text-muted">No launch methods available</span>';
                    
                    return `
                        <li class="engine-item">
                            <div class="engine-header">
                                <span class="engine-name">${engine.name}</span>
                                <span class="engine-version">${engine.version} (${engine.date})</span>
                            </div>
                            <div class="engine-methods">
                                ${launchMethodsHtml}
                            </div>
                        </li>
                    `;
                }).join('');
            }
        }
        
        window.allBackends = backends;
        
        // Populate model selects
        const qsModel = document.getElementById('qs-model');
        const benchModel = document.getElementById('bench-model');
        const dashboardModels = document.getElementById('dashboard-models');
        const modelsTableBody = document.getElementById('models-table-body');
        
        if (qsModel) {
            qsModel.innerHTML = models.map(m => 
                `<option value="${m.name}" data-quants="${(m.quantizations || []).join(',')}">${m.name}</option>`
            ).join('');
            updateQuantizationSelect(qsModel, 'qs-quant');
        }
        
        if (benchModel) {
            benchModel.innerHTML = models.map(m => 
                `<option value="${m.name}" data-quants="${(m.quantizations || []).join(',')}">${m.name}</option>`
            ).join('');
            updateQuantizationSelect(benchModel, 'bench-quant');
        }
        
        if (dashboardModels) {
            if (models.length === 0) {
                dashboardModels.innerHTML = '<p class="empty-state">No models downloaded yet.</p>';
            } else {
                const displayModels = models.slice(0, 10);
                dashboardModels.innerHTML = `
                    <table class="model-table compact">
                        <thead>
                            <tr><th>Model</th><th>Quantizations</th><th>Size</th><th>Actions</th></tr>
                        </thead>
                        <tbody>
                            ${displayModels.map(m => `
                                <tr>
                                    <td>${m.name}</td>
                                    <td>${(m.quantizations || []).map(q => `<span class="badge quant">${q}</span>`).join('')}</td>
                                    <td>${m.size_human || '-'}</td>
                                    <td><button onclick="startWithModel('${m.name}')" class="btn small">Start</button></td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                    ${models.length > 10 ? `<button onclick="navigateTo('/models')" class="btn secondary" style="margin-top: 12px;">View ${models.length - 10} more models</button>` : ''}
                `;
            }
        }
        
        if (modelsTableBody) {
            if (models.length === 0) {
                modelsTableBody.innerHTML = '<tr><td colspan="4" class="empty-state">No models downloaded yet.</td></tr>';
            } else {
                modelsTableBody.innerHTML = models.map(m => `
                    <tr>
                        <td>${m.name}</td>
                        <td>${(m.quantizations || []).map(q => `<span class="badge quant">${q}</span>`).join('')}</td>
                        <td>${m.size_human || '-'}</td>
                        <td>
                            <button onclick="startWithModel('${m.name}')" class="btn small">Start</button>
                            <button onclick="showBenchmarkModal('${m.name}')" class="btn small secondary">Benchmark</button>
                            <button onclick="deleteModel('${m.name}')" class="btn small danger">Delete</button>
                        </td>
                    </tr>
                `).join('');
            }
        }
        
        window.allModels = models;
    } catch (error) {
        console.error('Error loading dashboard data:', error);
    }
}

// Alias for compatibility
async function loadModels() {
    await loadDashboardData();
}

setInterval(() => {
    const settingsTab = document.getElementById('settings');
    const dashboardTab = document.getElementById('dashboard');
    if ((settingsTab && settingsTab.classList.contains('active')) ||
        (dashboardTab && dashboardTab.classList.contains('active'))) {
        loadSystemStatus();
        loadEngineStatus();
        loadEngines();
    }
}, 5000);


async function exportToOpenCode(serverId) {
    showConfirm('Export to OpenCode', 'Add this server and model to OpenCode configuration?', async () => {
        try {
            const response = await fetch(`/api/servers/${serverId}/export-opencode`, { method: 'POST' });
            const result = await response.json();
            
            if (response.ok && result.success) {
                showNotification('Exported to OpenCode successfully!', 'success');
            } else {
                showNotification('Failed to export to OpenCode: ' + (result.error || 'Unknown error'), 'error');
            }
        } catch (error) {
            console.error('Error exporting to OpenCode:', error);
            showNotification('Error exporting to OpenCode: ' + error.message, 'error');
        }
    });
}


// Context Scaling Functions
let scalingChartInstance = null;

function populateScalingSelect(results) {
    const select = document.getElementById('scaling-result-select');
    if (!select) return;
    
    // Filter for find-max-ctx-manual and ctx-scaling-benchmark types
    const scalingResults = results.filter(r => r.type === 'find-max-ctx-manual' || r.type === 'ctx-scaling-benchmark');
    
    // Keep current selection if possible
    const currentVal = select.value;
    
    select.innerHTML = '<option value="">Select a scaling benchmark result...</option>';
    scalingResults.forEach(r => {
        const option = document.createElement('option');
        option.value = r.id;
        const dateStr = new Date(r.timestamp).toLocaleString();
        const maxCtxStr = r.max_working_ctx ? ` (Max: ${r.max_working_ctx})` : '';
        option.textContent = `${r.model} - ${r.backend_type} - ${r.type}${maxCtxStr} [${dateStr}]`;
        select.appendChild(option);
    });
    
    if (currentVal && scalingResults.some(r => r.id === currentVal)) {
        select.value = currentVal;
    } else if (scalingResults.length > 0) {
        select.value = scalingResults[0].id; // Auto select first if none selected
    }
    
    renderScalingChart();
}

function renderScalingChart() {
    const select = document.getElementById('scaling-result-select');
    if (!select || !select.value) return;
    
    const resultId = select.value;
    const result = window.allBenchmarks?.find(r => r.id === resultId);
    
    if (!result || !result.probes) {
        if (scalingChartInstance) {
            scalingChartInstance.destroy();
            scalingChartInstance = null;
        }
        return;
    }
    
    const ctx = document.getElementById('scaling-chart').getContext('2d');
    
    // Process data for chart
    // Sort probes by ctx size
    const sortedProbes = [...result.probes].sort((a, b) => a.ctx - b.ctx);
    
    const labels = sortedProbes.map(p => p.ctx);
    const tgTpsData = sortedProbes.map(p => p.tg_tps || 0);
    const ppTpsData = sortedProbes.map(p => p.pp_tps || 0);
    
    if (scalingChartInstance) {
        scalingChartInstance.destroy();
    }
    
    scalingChartInstance = new Chart(ctx, {
        type: 'line',
        data: {
            labels: labels,
            datasets: [
                {
                    label: 'Token Generation TPS',
                    data: tgTpsData,
                    borderColor: 'rgb(75, 192, 192)',
                    tension: 0.1,
                    yAxisID: 'y'
                },
                {
                    label: 'Prompt Processing TPS',
                    data: ppTpsData,
                    borderColor: 'rgb(255, 99, 132)',
                    tension: 0.1,
                    yAxisID: 'y'
                }
            ]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            interaction: {
                mode: 'index',
                intersect: false,
            },
            scales: {
                x: {
                    title: {
                        display: true,
                        text: 'Context Size (Tokens)'
                    }
                },
                y: {
                    type: 'linear',
                    display: true,
                    position: 'left',
                    title: {
                        display: true,
                        text: 'Tokens Per Second (TPS)'
                    },
                    min: 0
                }
            },
            plugins: {
                title: {
                    display: true,
                    text: `Context Scaling: ${result.model} (${result.backend_type})`
                },
                tooltip: {
                    callbacks: {
                        afterBody: function(context) {
                            const dataIndex = context[0].dataIndex;
                            const probe = sortedProbes[dataIndex];
                            if (probe.error) {
                                return `Error: ${probe.error}`;
                            } else if (!probe.success) {
                                return `Status: FAILED`;
                            }
                            return `Status: SUCCESS`;
                        }
                    }
                }
            }
        }
    });
}

function runFindMaxCtx() {
    const formData = new FormData(document.getElementById('benchmark-form'));
    const config = Object.fromEntries(formData);
    const modelName = config.model;
    const modelPath = getModelPath(modelName, config.quantization || '');
    
    const max_ctx = prompt("Enter upper bound for context search:", "180224");
    if (!max_ctx) return;
    
    showNotification('Starting Find Max Context benchmark...', 'info');
    
    fetch('/api/benchmarks/find-max-ctx-manual', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            model_path: modelPath,
            model: modelName,
            backend: config.backend_type,
            vendor_backend: config.vendor_backend || 'auto',
            max_ctx: parseInt(max_ctx),
            n_gpu_layers: parseInt(config.n_gpu_layers)
        })
    }).then(response => response.json())
      .then(result => {
          if (result.error) {
              showNotification('Failed to start: ' + result.error, 'error');
          } else {
              showNotification('Find Max Context benchmark started!', 'success');
              setTimeout(loadBenchmarks, 1000);
          }
      }).catch(err => {
          showNotification('Error: ' + err.message, 'error');
      });
}

function runCtxScaling() {
    const formData = new FormData(document.getElementById('benchmark-form'));
    const config = Object.fromEntries(formData);
    const modelName = config.model;
    const modelPath = getModelPath(modelName, config.quantization || '');
    
    const steps_input = prompt("Enter context sizes to test (comma-separated):", "8192, 32768, 65536, 131072");
    if (!steps_input) return;
    
    const steps = steps_input.split(',').map(s => parseInt(s.trim())).filter(s => !isNaN(s));
    
    showNotification('Starting Context Scaling benchmark...', 'info');
    
    fetch('/api/benchmarks/ctx-scaling', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            model_path: modelPath,
            model: modelName,
            backend: config.backend_type,
            vendor_backend: config.vendor_backend || 'auto',
            ctx_sizes: steps,
            n_gpu_layers: parseInt(config.n_gpu_layers)
        })
    }).then(response => response.json())
      .then(result => {
          if (result.error) {
              showNotification('Failed to start: ' + result.error, 'error');
          } else {
              showNotification('Context Scaling benchmark started!', 'success');
              setTimeout(loadBenchmarks, 1000);
          }
      }).catch(err => {
          showNotification('Error: ' + err.message, 'error');
      });
}


// Storage Management Functions
async function loadLibraries() {
    try {
        const response = await fetch('/api/libraries');
        const libraries = await response.json();
        const container = document.getElementById('libraries-list');
        if (!container) return;
        
        container.innerHTML = '';
        libraries.forEach(lib => {
            const div = document.createElement('div');
            div.className = 'server-item';
            div.innerHTML = `
                <div class="server-info">
                    <span class="server-name">${lib.name || lib.id}</span>
                    <span class="badge ${lib.id.includes('default') ? 'primary' : 'secondary'}">${lib.path}</span>
                </div>
                <div class="server-actions">
                    <button onclick="unregisterLibrary('${lib.id}')" class="btn small danger" ${lib.id === 'default' ? 'disabled' : ''}>Unregister</button>
                </div>
            `;
            container.appendChild(div);
        });
        
        window.allLibraries = libraries;
        loadMoveModelsList();
    } catch (error) {
        console.error('Error loading libraries:', error);
    }
}

async function registerLibrary() {
    const path = document.getElementById('lib-path').value;
    const name = document.getElementById('lib-name').value;
    
    if (!path) {
        showNotification('Path is required', 'error');
        return;
    }
    
    try {
        const response = await fetch('/api/libraries', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path, name })
        });
        const result = await response.json();
        if (result.success) {
            showNotification('Library registered successfully', 'success');
            document.getElementById('lib-path').value = '';
            document.getElementById('lib-name').value = '';
            loadLibraries();
        } else {
            showNotification('Error: ' + result.error, 'error');
        }
    } catch (error) {
        showNotification('Error registering library: ' + error.message, 'error');
    }
}

async function unregisterLibrary(libId) {
    if (!confirm('Are you sure you want to unregister this library?')) return;
    
    try {
        const response = await fetch(`/api/libraries/${libId}`, { method: 'DELETE' });
        const result = await response.json();
        if (result.success) {
            showNotification('Library unregistered', 'success');
            loadLibraries();
        }
    } catch (error) {
        showNotification('Error unregistering library: ' + error.message, 'error');
    }
}

async function loadMoveModelsList() {
    try {
        const response = await fetch('/api/models');
        const models = await response.json();
        const container = document.getElementById('move-models-list');
        if (!container) return;
        
        container.innerHTML = '';
        models.forEach(model => {
            const tr = document.createElement('tr');
            
            // Build options for target library
            let options = '<option value="">Select Target...</option>';
            window.allLibraries.forEach(lib => {
                if (!model.path.startsWith(lib.path)) {
                    options += `<option value="${lib.id}">${lib.name || lib.id}</option>`;
                }
            });
            
            tr.innerHTML = `
                <td>${model.name}</td>
                <td><small>${model.path}</small></td>
                <td>
                    <select id="target-lib-${model.name.replace(/[^a-z0-9]/gi, '_')}">
                        ${options}
                    </select>
                </td>
                <td>
                    <button onclick="moveModel('${model.name}', '${model.path}')" class="btn small">Move</button>
                </td>
            `;
            container.appendChild(tr);
        });
    } catch (error) {
        console.error('Error loading models for move:', error);
    }
}

async function moveModel(modelName, sourcePath) {
    const safeName = modelName.replace(/[^a-z0-9]/gi, '_');
    const targetLibId = document.getElementById(`target-lib-${safeName}`).value;
    
    if (!targetLibId) {
        showNotification('Please select a target library', 'error');
        return;
    }
    
    showNotification('Moving model... please wait.', 'info');
    
    try {
        const response = await fetch('/api/models/move', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                model_name: modelName,
                source_path: sourcePath,
                target_library_id: targetLibId
            })
        });
        const result = await response.json();
        if (result.success) {
            showNotification('Model moved successfully', 'success');
            loadLibraries(); // Refresh lists
            loadModels(); // Refresh models tab
        } else {
            showNotification('Error: ' + result.error, 'error');
        }
    } catch (error) {
        showNotification('Error moving model: ' + error.message, 'error');
    }
}

// =============================================================================
// Authentication Handling (Simple Local Auth)
// =============================================================================

let authStatus = null;

async function checkAuthStatus() {
    try {
        const response = await fetch('/api/auth/status');
        const data = await response.json();
        authStatus = data;
        return data;
    } catch (error) {
        console.error('Error checking auth status:', error);
        return null;
    }
}

// Logout function
async function logout() {
    try {
        await fetch('/api/auth/logout', { method: 'POST' });
    } catch (error) {
        console.error('Logout error:', error);
    }
    window.location.href = '/auth';
}
