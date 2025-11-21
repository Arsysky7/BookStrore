// /pdf-bookstore/admin-panel/js/book-manager.js

// Enterprise-Grade Book Manager for Complete CRUD Operations
class BookManager {
    constructor() {
        this.api = new AdminAPI(); // From admin-api.js line 3
        this.auth = new AdminAuthentication(); // From admin-auth.js line 3

        // Enterprise state management with error handling
        this.state = {
            books: [],
            filteredBooks: [],
            currentPage: 1,
            itemsPerPage: 25,
            totalBooks: 0,
            sortBy: 'created_at',
            sortOrder: 'desc',
            currentView: 'table',
            filters: {
                search: '',
                status: '',
                category: ''
            },
            selectedBooks: new Set(),
            categories: [],
            isLoading: false,
            error: null,
            retryCount: 0,
            maxRetries: 3
        };

        // Professional edit modal state
        this.editModal = {
            isOpen: false,
            currentBook: null,
            categories: []
        };

        // Enterprise delete modal state
        this.deleteModal = null;

        this.initializeBookManager();
    }

    // Enterprise initialization with comprehensive error handling
    async initializeBookManager() {
        try {
            // Verify admin access with professional error handling
            await this.verifyAdminAccess();

            // Setup event listeners with error boundaries
            this.setupEventListeners();

            // Load initial data with retry mechanism
            await this.loadInitialData();

            // Setup real-time updates
            this.setupRealTimeUpdates();

            Utils.showNotification('Book management loaded successfully', 'success');

        } catch (error) {
            console.error('Book manager initialization failed:', error);
            const errorMessage = (error && error.message) || 'Unknown initialization error';
            Utils.showNotification('Failed to initialize book management: ' + errorMessage, 'error');

            if (errorMessage.includes('Admin') || errorMessage.includes('authentication')) {
                setTimeout(() => {
                    window.location.href = 'index.html';
                }, 2000);
            }
        }
    }

    // Professional admin access verification
    async verifyAdminAccess() {
        const isAuthenticated = this.auth && this.auth.isAdminAuthenticated && this.auth.isAdminAuthenticated();

        if (!isAuthenticated) {
            throw new Error('Admin authentication required');
        }

        const currentAdmin = this.auth && this.auth.getCurrentAdmin && this.auth.getCurrentAdmin();
        const adminData = (currentAdmin && currentAdmin.data) || currentAdmin;

        if (!adminData) {
            throw new Error('Admin user data not available');
        }

        this.updateAdminProfile(adminData);
    }

    // Enterprise initial data loading with parallel requests
    async loadInitialData() {
        try {
            Utils.showLoading && Utils.showLoading('Loading book data...');

            // Load data with parallel requests for performance
            const dataPromises = [
                this.loadBooks().catch(error => {
                    console.warn('Books loading failed:', error);
                    return null;
                }),
                this.loadBookStats().catch(error => {
                    console.warn('Stats loading failed:', error);
                    return null;
                }),
                this.loadCategories().catch(error => {
                    console.warn('Categories loading failed:', error);
                    return [];
                })
            ];

            const [booksData, bookStats, categories] = await Promise.all(dataPromises);

            // Update statistics safely
            if (bookStats) {
                this.updateBookStatistics(bookStats);
            }

            // Populate category filter safely
            if (categories && categories.length > 0) {
                this.populateCategoryFilter(categories);
            }

            Utils.hideLoading && Utils.hideLoading();

        } catch (error) {
            Utils.hideLoading && Utils.hideLoading();
            console.error('Failed to load initial data:', error);
            Utils.showNotification('Some data failed to load', 'warning');
        }
    }

    // Professional book loading with pagination and filtering
    async loadBooks() {
        try {
            this.state.isLoading = true;
            this.showLoadingState();

            // Build query parameters safely
            const params = new URLSearchParams();

            // Add pagination parameters
            params.set('page', this.state.currentPage.toString());
            params.set('per_page', this.state.itemsPerPage.toString());
            params.set('sort_by', this.state.sortBy || 'created_at');
            params.set('sort_order', this.state.sortOrder || 'desc');

            // Add filter parameters
            const filters = this.state.filters || {};
            Object.entries(filters).forEach(([key, value]) => {
                if (value && value.toString().trim()) {
                    params.set(key, value.toString().trim());
                }
            });

            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;

            if (!apiEndpoint || !apiRequest) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books?' + params.toString());
            // API call to book-service/handlers.rs line 156

            const responseData = (response && response.data) || {};
            if (response && response.success && responseData) {
                this.state.books = Array.isArray(responseData.books) ? responseData.books : [];
                this.state.totalBooks = parseInt(responseData.total) || 0;

                // Update UI safely
                this.renderBooks();
                this.updatePagination();
                this.updateBookCount();
                this.hideLoadingState();
            } else {
                throw new Error('Invalid books response');
            }

        } catch (error) {
            console.error('Failed to load books:', error);
            this.hideLoadingState();
            this.showEmptyState();
            const errorMessage = (error && error.message) || 'Unknown error';
            Utils.showNotification('Failed to load books: ' + errorMessage, 'error');
        } finally {
            this.state.isLoading = false;
        }
    }

    // Professional book statistics loading
    async loadBookStats() {
        try {
            const apiGetStats = this.api && this.api.getAdminBookStats;

            if (!apiGetStats) {
                console.warn('Book stats API not available');
                return null;
            }

            const stats = await apiGetStats();
            return stats;

        } catch (error) {
            console.error('Failed to load book stats:', error);
            return null;
        }
    }

    // Enterprise categories loading with error handling
    async loadCategories() {
        try {
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;

            if (!apiEndpoint || !apiRequest) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(apiEndpoint + '/categories');

            const responseData = (response && response.data) || [];
            if (response && response.success && responseData) {
                this.state.categories = Array.isArray(responseData) ? responseData : [];
                return this.state.categories;
            }

            return [];
        } catch (error) {
            console.error('Failed to load categories:', error);
            return [];
        }
    }

    // Enterprise event listeners setup with error boundaries
    setupEventListeners() {
        try {
            this.setupSearchAndFiltering();
            this.setupSorting();
            this.setupViewToggle();
            this.setupPagination();
            this.setupBookActions();
            this.setupBulkActions();
            this.setupModalEvents();
            this.setupUIEvents();
        } catch (error) {
            console.error('Event listener setup failed:', error);
            Utils.showNotification('Some features may not work properly', 'warning');
        }
    }

    // Professional search and filtering setup
    setupSearchAndFiltering() {
        // Search input with debouncing
        const searchInput = document.getElementById('book-search');
        if (searchInput) {
            const debouncedSearch = Utils.debounce && Utils.debounce(() => {
                try {
                    const searchValue = searchInput.value || '';
                    this.state.filters.search = searchValue.trim();
                    this.state.currentPage = 1;
                    this.loadBooks();
                } catch (error) {
                    console.error('Search failed:', error);
                }
            }, 300) || (() => {
                // Fallback if Utils.debounce not available
                const searchValue = searchInput.value || '';
                this.state.filters.search = searchValue.trim();
                this.state.currentPage = 1;
                this.loadBooks();
            });

            searchInput.addEventListener('input', debouncedSearch);
        }

        // Search submit button
        const searchSubmit = document.getElementById('search-submit');
        if (searchSubmit) {
            searchSubmit.addEventListener('click', () => {
                try {
                    this.loadBooks();
                } catch (error) {
                    console.error('Search submit failed:', error);
                }
            });
        }

        // Search clear button
        const searchClear = document.getElementById('search-clear');
        if (searchClear) {
            searchClear.addEventListener('click', () => {
                try {
                    if (searchInput) searchInput.value = '';
                    this.state.filters.search = '';
                    this.state.currentPage = 1;
                    this.loadBooks();
                    searchClear.style.display = 'none';
                } catch (error) {
                    console.error('Search clear failed:', error);
                }
            });
        }

        // Status filter
        const statusFilter = document.getElementById('status-filter');
        if (statusFilter) {
            statusFilter.addEventListener('change', () => {
                try {
                    this.state.filters.status = statusFilter.value || '';
                    this.state.currentPage = 1;
                    this.loadBooks();
                } catch (error) {
                    console.error('Status filter failed:', error);
                }
            });
        }

        // Category filter
        const categoryFilter = document.getElementById('category-filter');
        if (categoryFilter) {
            categoryFilter.addEventListener('change', () => {
                try {
                    this.state.filters.category = categoryFilter.value || '';
                    this.state.currentPage = 1;
                    this.loadBooks();
                } catch (error) {
                    console.error('Category filter failed:', error);
                }
            });
        }

        // Items per page selector
        const itemsPerPage = document.getElementById('items-per-page');
        if (itemsPerPage) {
            itemsPerPage.addEventListener('change', () => {
                try {
                    const itemCount = parseInt(itemsPerPage.value) || 25;
                    this.state.itemsPerPage = itemCount;
                    this.state.currentPage = 1;
                    this.loadBooks();
                } catch (error) {
                    console.error('Items per page change failed:', error);
                }
            });
        }

        // Show search clear button when search has value
        if (searchInput && searchClear) {
            searchInput.addEventListener('input', () => {
                try {
                    const hasValue = searchInput.value && searchInput.value.trim();
                    searchClear.style.display = hasValue ? 'block' : 'none';
                } catch (error) {
                    console.error('Search clear toggle failed:', error);
                }
            });
        }
    }

    // Professional sorting functionality
    setupSorting() {
        const sortableHeaders = document.querySelectorAll('.sortable');

        sortableHeaders.forEach(header => {
            header.addEventListener('click', () => {
                try {
                    const sortField = header.dataset && header.dataset.sort;

                    if (!sortField) return;

                    // Toggle sort order if same field
                    if (this.state.sortBy === sortField) {
                        this.state.sortOrder = this.state.sortOrder === 'asc' ? 'desc' : 'asc';
                    } else {
                        this.state.sortBy = sortField;
                        this.state.sortOrder = 'asc';
                    }

                    // Update visual indicators
                    this.updateSortIndicators();

                    // Reload books with new sorting
                    this.state.currentPage = 1;
                    this.loadBooks();

                } catch (error) {
                    console.error('Sorting failed:', error);
                }
            });
        });
    }

    // Professional view toggle setup
    setupViewToggle() {
        const tableViewBtn = document.getElementById('table-view');
        const gridViewBtn = document.getElementById('grid-view');

        if (tableViewBtn) {
            tableViewBtn.addEventListener('click', () => {
                try {
                    this.switchView('table');
                } catch (error) {
                    console.error('Table view switch failed:', error);
                }
            });
        }

        if (gridViewBtn) {
            gridViewBtn.addEventListener('click', () => {
                try {
                    this.switchView('grid');
                } catch (error) {
                    console.error('Grid view switch failed:', error);
                }
            });
        }
    }

    // Enterprise pagination setup
    setupPagination() {
        // Pagination will be setup when updatePagination() is called
        // since pagination controls are generated dynamically
    }

    // Enterprise book actions setup with event delegation
    setupBookActions() {
        // Event delegation for book actions with comprehensive error handling
        document.addEventListener('click', async(e) => {
            try {
                const target = e.target;

                // Edit book action
                if (target.matches('.btn-edit-book') || target.closest('.btn-edit-book')) {
                    const editBtn = target.matches('.btn-edit-book') ? target : target.closest('.btn-edit-book');
                    const bookId = editBtn && editBtn.dataset && editBtn.dataset.bookId;

                    if (bookId) {
                        await this.openEditModal(bookId);
                    }
                }

                // Delete book action
                if (target.matches('.btn-delete-book') || target.closest('.btn-delete-book')) {
                    const deleteBtn = target.matches('.btn-delete-book') ? target : target.closest('.btn-delete-book');
                    const bookId = deleteBtn && deleteBtn.dataset && deleteBtn.dataset.bookId;

                    if (bookId) {
                        await this.openDeleteModal(bookId);
                    }
                }

                // Toggle status action
                if (target.matches('.btn-toggle-status') || target.closest('.btn-toggle-status')) {
                    const toggleBtn = target.matches('.btn-toggle-status') ? target : target.closest('.btn-toggle-status');
                    const bookId = toggleBtn && toggleBtn.dataset && toggleBtn.dataset.bookId;

                    if (bookId) {
                        await this.toggleBookStatus(bookId);
                    }
                }

                // View book action
                if (target.matches('.btn-view-book') || target.closest('.btn-view-book')) {
                    const viewBtn = target.matches('.btn-view-book') ? target : target.closest('.btn-view-book');
                    const bookId = viewBtn && viewBtn.dataset && viewBtn.dataset.bookId;

                    if (bookId) {
                        this.viewBookInStore(bookId);
                    }
                }

            } catch (error) {
                console.error('Book action failed:', error);
                Utils.showNotification('Action failed: ' + (error.message || 'Unknown error'), 'error');
            }
        });

        // Refresh button with loading state
        const refreshBtn = document.getElementById('refresh-books');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', async() => {
                try {
                    refreshBtn.disabled = true;
                    refreshBtn.innerHTML = '<i class="fas fa-spinner fa-spin"></i>';

                    await this.loadBooks();

                    refreshBtn.disabled = false;
                    refreshBtn.innerHTML = '<i class="fas fa-refresh"></i>';
                } catch (error) {
                    refreshBtn.disabled = false;
                    refreshBtn.innerHTML = '<i class="fas fa-refresh"></i>';
                    console.error('Refresh failed:', error);
                }
            });
        }

        // Export button with error handling
        const exportBtn = document.getElementById('export-books');
        if (exportBtn) {
            exportBtn.addEventListener('click', async() => {
                try {
                    await this.exportBooks();
                } catch (error) {
                    console.error('Export failed:', error);
                }
            });
        }
    }

    // Professional bulk actions setup
    setupBulkActions() {
        // Select all checkbox with error handling
        const selectAllCheckbox = document.getElementById('select-all');
        if (selectAllCheckbox) {
            selectAllCheckbox.addEventListener('change', () => {
                try {
                    this.toggleSelectAll(selectAllCheckbox.checked);
                } catch (error) {
                    console.error('Select all failed:', error);
                }
            });
        }

        // Bulk actions toggle
        const bulkActionsToggle = document.getElementById('bulk-actions-toggle');
        const bulkActionsMenu = document.getElementById('bulk-actions-menu');

        if (bulkActionsToggle && bulkActionsMenu) {
            bulkActionsToggle.addEventListener('click', () => {
                try {
                    const isVisible = bulkActionsMenu.style.display === 'block';
                    bulkActionsMenu.style.display = isVisible ? 'none' : 'block';
                } catch (error) {
                    console.error('Bulk actions toggle failed:', error);
                }
            });
        }

        // Bulk action buttons with comprehensive error handling
        document.addEventListener('click', async(e) => {
            try {
                if (e.target.matches('.bulk-action')) {
                    const action = e.target.dataset && e.target.dataset.action;
                    if (action) {
                        await this.handleBulkAction(action);
                    }
                }
            } catch (error) {
                console.error('Bulk action failed:', error);
                Utils.showNotification('Bulk action failed: ' + (error.message || 'Unknown error'), 'error');
            }
        });

        // Close bulk menu when clicking outside
        document.addEventListener('click', (e) => {
            try {
                if (bulkActionsMenu &&
                    !e.target.closest('#bulk-actions-toggle') &&
                    !e.target.closest('#bulk-actions-menu')) {
                    bulkActionsMenu.style.display = 'none';
                }
            } catch (error) {
                console.error('Bulk menu close failed:', error);
            }
        });
    }

    // Enterprise modal events setup
    setupModalEvents() {
        // Edit modal close events
        const modalClose = document.getElementById('modal-close');
        const modalCancel = document.getElementById('modal-cancel');

        if (modalClose) {
            modalClose.addEventListener('click', () => {
                try {
                    this.closeEditModal();
                } catch (error) {
                    console.error('Modal close failed:', error);
                }
            });
        }

        if (modalCancel) {
            modalCancel.addEventListener('click', () => {
                try {
                    this.closeEditModal();
                } catch (error) {
                    console.error('Modal cancel failed:', error);
                }
            });
        }

        // Edit form submission
        const editForm = document.getElementById('edit-book-form');
        if (editForm) {
            editForm.addEventListener('submit', async(e) => {
                e.preventDefault();
                try {
                    await this.handleBookUpdate();
                } catch (error) {
                    console.error('Book update failed:', error);
                }
            });
        }

        // Delete modal events
        const deleteModalClose = document.getElementById('delete-modal-close');
        const deleteCancel = document.getElementById('delete-cancel');
        const confirmDelete = document.getElementById('confirm-delete');

        if (deleteModalClose) {
            deleteModalClose.addEventListener('click', () => {
                try {
                    this.closeDeleteModal();
                } catch (error) {
                    console.error('Delete modal close failed:', error);
                }
            });
        }

        if (deleteCancel) {
            deleteCancel.addEventListener('click', () => {
                try {
                    this.closeDeleteModal();
                } catch (error) {
                    console.error('Delete cancel failed:', error);
                }
            });
        }

        if (confirmDelete) {
            confirmDelete.addEventListener('click', async() => {
                try {
                    await this.handleBookDelete();
                } catch (error) {
                    console.error('Book delete failed:', error);
                }
            });
        }

        // Close modal when clicking outside
        document.addEventListener('click', (e) => {
            try {
                if (e.target.matches('.modal')) {
                    this.closeEditModal();
                    this.closeDeleteModal();
                }
            } catch (error) {
                console.error('Modal outside click failed:', error);
            }
        });
    }

    // Professional UI events setup
    setupUIEvents() {
        try {
            this.setupSidebarToggle();
            this.setupMobileMenuToggle();
            this.setupLogoutHandler();
        } catch (error) {
            console.error('UI events setup failed:', error);
        }
    }

    // Professional sidebar toggle
    setupSidebarToggle() {
        const sidebarToggle = document.getElementById('sidebar-toggle');
        const sidebar = document.getElementById('admin-sidebar');

        if (sidebarToggle && sidebar) {
            sidebarToggle.addEventListener('click', () => {
                try {
                    sidebar.classList.toggle('collapsed');
                } catch (error) {
                    console.error('Sidebar toggle failed:', error);
                }
            });
        }
    }

    // Professional mobile menu toggle
    setupMobileMenuToggle() {
        const mobileMenuToggle = document.getElementById('mobile-menu-toggle');
        const sidebar = document.getElementById('admin-sidebar');

        if (mobileMenuToggle && sidebar) {
            mobileMenuToggle.addEventListener('click', () => {
                try {
                    sidebar.classList.toggle('mobile-open');
                } catch (error) {
                    console.error('Mobile menu toggle failed:', error);
                }
            });
        }
    }

    // Professional logout handler
    setupLogoutHandler() {
        const logoutBtn = document.getElementById('admin-logout');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', async(e) => {
                e.preventDefault();
                try {
                    await this.handleLogout();
                } catch (error) {
                    console.error('Logout failed:', error);
                }
            });
        }
    }

    // Enterprise books rendering with error handling
    renderBooks() {
        try {
            if (this.state.currentView === 'table') {
                this.renderTableView();
            } else {
                this.renderGridView();
            }

            // Update book selection state
            this.updateSelectionState();

        } catch (error) {
            console.error('Books rendering failed:', error);
            this.showEmptyState();
            Utils.showNotification('Display error occurred', 'warning');
        }
    }

    // Professional table view rendering
    renderTableView() {
        const tableBody = document.getElementById('books-table-body');
        if (!tableBody) return;

        const books = this.state.books || [];
        if (books.length === 0) {
            this.showEmptyState();
            return;
        }

        try {
            const tableRows = books.map(book => {
                const bookData = book || {};
                const bookId = (bookData.id || '').toString();
                const title = bookData.title || 'Untitled';
                const author = bookData.author || 'Unknown Author';
                const price = parseFloat(bookData.price) || 0;
                const description = bookData.description || '';
                const coverPath = bookData.cover_path || '/assets/default-cover.jpg';
                const categories = bookData.categories || [];
                const downloadCount = parseInt(bookData.download_count) || 0;
                const isActive = Boolean(bookData.is_active);
                const createdAt = bookData.created_at || '';

                return `
                    <tr class="book-row" data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}">
                        <td class="checkbox-col">
                            <input type="checkbox" class="table-checkbox book-checkbox" data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}">
                        </td>
                        <td class="book-title-col">
                            <div class="book-title-info">
                                <img src="${Utils.escapeHtml && Utils.escapeHtml(coverPath) || coverPath}" 
                                     alt="${Utils.escapeHtml && Utils.escapeHtml(title) || title}" 
                                     class="book-thumbnail" 
                                     loading="lazy"
                                     onerror="this.src='/assets/default-cover.jpg'">
                                <div class="book-title-text">
                                    <h4 class="book-title">${Utils.escapeHtml && Utils.escapeHtml(title) || title}</h4>
                                    <p class="book-description">${Utils.escapeHtml && Utils.escapeHtml(Utils.truncateText && Utils.truncateText(description, 100) || description.substring(0, 100)) || description}</p>
                                </div>
                            </div>
                        </td>
                        <td class="book-author">${Utils.escapeHtml && Utils.escapeHtml(author) || author}</td>
                        <td class="book-price">${Utils.formatCurrency && Utils.formatCurrency(price) || ('IDR ' + price.toLocaleString())}</td>
                        <td class="book-categories">
                            ${this.renderBookCategories(categories)}
                        </td>
                        <td class="book-downloads">
                            <span class="download-count">${downloadCount.toLocaleString()}</span>
                        </td>
                        <td class="book-status">
                            <span class="status-badge ${isActive ? 'status-active' : 'status-inactive'}">
                                ${isActive ? 'Active' : 'Inactive'}
                            </span>
                        </td>
                        <td class="book-created">${Utils.formatDate && Utils.formatDate(createdAt) || createdAt}</td>
                        <td class="book-actions">
                            <div class="action-buttons">
                                <button class="btn btn-sm btn-primary btn-edit-book" 
                                        data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}" 
                                        title="Edit Book">
                                    <i class="fas fa-edit"></i>
                                </button>
                                <button class="btn btn-sm ${isActive ? 'btn-warning' : 'btn-success'} btn-toggle-status" 
                                        data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}" 
                                        title="${isActive ? 'Deactivate' : 'Activate'} Book">
                                    <i class="fas fa-${isActive ? 'eye-slash' : 'eye'}"></i>
                                </button>
                                <button class="btn btn-sm btn-info btn-view-book" 
                                        data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}" 
                                        title="View in Store">
                                    <i class="fas fa-external-link-alt"></i>
                                </button>
                                <button class="btn btn-sm btn-danger btn-delete-book" 
                                        data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}" 
                                        title="Delete Book">
                                    <i class="fas fa-trash"></i>
                                </button>
                            </div>
                        </td>
                    </tr>
                `;
            }).join('');

            tableBody.innerHTML = tableRows;

            // Setup checkbox event listeners
            this.setupBookCheckboxes();

        } catch (error) {
            console.error('Table view rendering failed:', error);
            tableBody.innerHTML = '<tr><td colspan="9" class="text-center">Error rendering books</td></tr>';
        }
    }

    // Professional grid view rendering
    renderGridView() {
        const gridContainer = document.getElementById('books-grid');
        if (!gridContainer) return;

        const books = this.state.books || [];
        if (books.length === 0) {
            this.showEmptyState();
            return;
        }

        try {
            const gridCards = books.map(book => {
                const bookData = book || {};
                const bookId = (bookData.id || '').toString();
                const title = bookData.title || 'Untitled';
                const author = bookData.author || 'Unknown Author';
                const price = parseFloat(bookData.price) || 0;
                const coverPath = bookData.cover_path || '/assets/default-cover.jpg';
                const downloadCount = parseInt(bookData.download_count) || 0;
                const isActive = Boolean(bookData.is_active);
                const createdAt = bookData.created_at || '';
                const categories = bookData.categories || [];

                return `
                    <div class="book-card" data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}">
                        <div class="book-card-header">
                            <input type="checkbox" class="book-checkbox" data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}">
                            <span class="status-badge ${isActive ? 'status-active' : 'status-inactive'}">
                                ${isActive ? 'Active' : 'Inactive'}
                            </span>
                        </div>
                        <div class="book-cover">
                            <img src="${Utils.escapeHtml && Utils.escapeHtml(coverPath) || coverPath}" 
                                 alt="${Utils.escapeHtml && Utils.escapeHtml(title) || title}" 
                                 loading="lazy"
                                 onerror="this.src='/assets/default-cover.jpg'">
                        </div>
                        <div class="book-info">
                            <h4 class="book-title">${Utils.escapeHtml && Utils.escapeHtml(title) || title}</h4>
                            <p class="book-author">by ${Utils.escapeHtml && Utils.escapeHtml(author) || author}</p>
                            <p class="book-price">${Utils.formatCurrency && Utils.formatCurrency(price) || ('IDR ' + price.toLocaleString())}</p>
                            <div class="book-stats">
                                <span class="stat-item">
                                    <i class="fas fa-download"></i>
                                    ${downloadCount.toLocaleString()} downloads
                                </span>
                                <span class="stat-item">
                                    <i class="fas fa-calendar"></i>
                                    ${Utils.formatRelativeTime && Utils.formatRelativeTime(createdAt) || createdAt}
                                </span>
                            </div>
                            ${this.renderBookCategories(categories)}
                        </div>
                        <div class="book-actions">
                            <button class="btn btn-sm btn-primary btn-edit-book" 
                                    data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}" 
                                    title="Edit Book">
                                <i class="fas fa-edit"></i> Edit
                            </button>
                            <button class="btn btn-sm ${isActive ? 'btn-warning' : 'btn-success'} btn-toggle-status" 
                                    data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}" 
                                    title="${isActive ? 'Deactivate' : 'Activate'} Book">
                                <i class="fas fa-${isActive ? 'eye-slash' : 'eye'}"></i>
                            </button>
                            <button class="btn btn-sm btn-danger btn-delete-book" 
                                    data-book-id="${Utils.escapeHtml && Utils.escapeHtml(bookId) || bookId}" 
                                    title="Delete Book">
                                <i class="fas fa-trash"></i>
                            </button>
                        </div>
                    </div>
                `;
            }).join('');

            gridContainer.innerHTML = gridCards;

            // Setup checkbox event listeners
            this.setupBookCheckboxes();

        } catch (error) {
            console.error('Grid view rendering failed:', error);
            gridContainer.innerHTML = '<div class="text-center">Error rendering books</div>';
        }
    }

    // Professional book categories rendering
    renderBookCategories(categories) {
        try {
            const categoryList = Array.isArray(categories) ? categories : [];

            if (categoryList.length === 0) {
                return '<span class="no-categories">No categories</span>';
            }

            return categoryList.map(category => {
                const categoryData = category || {};
                const categoryName = categoryData.name || category || 'Unknown Category';

                return `<span class="category-tag">${Utils.escapeHtml && Utils.escapeHtml(categoryName) || categoryName}</span>`;
            }).join('');

        } catch (error) {
            console.error('Categories rendering failed:', error);
            return '<span class="no-categories">Categories error</span>';
        }
    }

    // Enterprise book checkboxes setup
    setupBookCheckboxes() {
        const checkboxes = document.querySelectorAll('.book-checkbox');

        checkboxes.forEach(checkbox => {
            checkbox.addEventListener('change', () => {
                try {
                    const bookId = checkbox.dataset && checkbox.dataset.bookId;

                    if (!bookId) return;

                    if (checkbox.checked) {
                        this.state.selectedBooks.add(bookId);
                    } else {
                        this.state.selectedBooks.delete(bookId);
                    }

                    this.updateSelectionState();

                } catch (error) {
                    console.error('Checkbox change failed:', error);
                }
            });
        });
    }

    // Professional view switching
    switchView(viewType) {
        try {
            this.state.currentView = viewType;

            // Update view toggle buttons
            const tableBtn = document.getElementById('table-view');
            const gridBtn = document.getElementById('grid-view');

            if (tableBtn && gridBtn) {
                tableBtn.classList.toggle('active', viewType === 'table');
                gridBtn.classList.toggle('active', viewType === 'grid');
            }

            // Show/hide appropriate containers
            const tableContainer = document.getElementById('table-view-container');
            const gridContainer = document.getElementById('grid-view-container');

            if (tableContainer) {
                tableContainer.style.display = viewType === 'table' ? 'block' : 'none';
            }

            if (gridContainer) {
                gridContainer.style.display = viewType === 'grid' ? 'block' : 'none';
            }

            // Re-render books
            this.renderBooks();

            // Save view preference
            try {
                localStorage.setItem('admin_book_view', viewType);
            } catch (storageError) {
                console.warn('Failed to save view preference:', storageError);
            }

        } catch (error) {
            console.error('View switch failed:', error);
        }
    }

    // Enterprise select all toggle
    toggleSelectAll(selectAll) {
        try {
            this.state.selectedBooks.clear();

            if (selectAll) {
                const books = this.state.books || [];
                books.forEach(book => {
                    const bookId = book && book.id;
                    if (bookId) {
                        this.state.selectedBooks.add(bookId.toString());
                    }
                });
            }

            // Update checkboxes
            const checkboxes = document.querySelectorAll('.book-checkbox');
            checkboxes.forEach(checkbox => {
                checkbox.checked = selectAll;
            });

            this.updateSelectionState();

        } catch (error) {
            console.error('Select all toggle failed:', error);
        }
    }

    // Professional selection state update
    updateSelectionState() {
        try {
            const selectedCount = this.state.selectedBooks.size;
            const totalCount = (this.state.books || []).length;

            // Update select all checkbox
            const selectAllCheckbox = document.getElementById('select-all');
            if (selectAllCheckbox) {
                selectAllCheckbox.indeterminate = selectedCount > 0 && selectedCount < totalCount;
                selectAllCheckbox.checked = selectedCount === totalCount && totalCount > 0;
            }

            // Show/hide bulk actions based on selection
            const bulkActionsToggle = document.getElementById('bulk-actions-toggle');
            if (bulkActionsToggle) {
                bulkActionsToggle.style.display = selectedCount > 0 ? 'inline-flex' : 'none';
                bulkActionsToggle.textContent = 'Bulk Actions (' + selectedCount + ')';
            }

        } catch (error) {
            console.error('Selection state update failed:', error);
        }
    }

    // Enterprise bulk action handling
    async handleBulkAction(action) {
        try {
            const selectedIds = Array.from(this.state.selectedBooks);

            if (selectedIds.length === 0) {
                Utils.showNotification('No books selected', 'warning');
                return;
            }

            const confirmMessage = this.getBulkActionConfirmMessage(action, selectedIds.length);
            if (!confirm(confirmMessage)) {
                return;
            }

            Utils.showLoading && Utils.showLoading('Processing ' + action + ' for ' + selectedIds.length + ' books...');

            switch (action) {
                case 'activate':
                    await this.bulkUpdateStatus(selectedIds, true);
                    break;
                case 'deactivate':
                    await this.bulkUpdateStatus(selectedIds, false);
                    break;
                case 'delete':
                    await this.bulkDeleteBooks(selectedIds);
                    break;
                default:
                    throw new Error('Unknown bulk action: ' + action);
            }

            // Reload books
            await this.loadBooks();

            // Clear selection
            this.state.selectedBooks.clear();
            this.updateSelectionState();

            Utils.hideLoading && Utils.hideLoading();
            Utils.showNotification('Bulk ' + action + ' completed successfully', 'success');

        } catch (error) {
            Utils.hideLoading && Utils.hideLoading();
            console.error('Bulk action failed:', error);
            const errorMessage = (error && error.message) || 'Unknown error';
            Utils.showNotification('Bulk ' + action + ' failed: ' + errorMessage, 'error');
        }
    }

    // Professional bulk action confirmation message
    getBulkActionConfirmMessage(action, count) {
        const bookText = count === 1 ? 'book' : 'books';

        switch (action) {
            case 'activate':
                return 'Are you sure you want to activate ' + count + ' ' + bookText + '?';
            case 'deactivate':
                return 'Are you sure you want to deactivate ' + count + ' ' + bookText + '?';
            case 'delete':
                return 'Are you sure you want to delete ' + count + ' ' + bookText + '? This action cannot be undone.';
            default:
                return 'Are you sure you want to perform this action on ' + count + ' ' + bookText + '?';
        }
    }

    // Enterprise bulk status update
    async bulkUpdateStatus(bookIds, isActive) {
        try {
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;

            if (!apiEndpoint || !apiRequest) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books/bulk-status', {
                method: 'PUT',
                body: JSON.stringify({
                    book_ids: bookIds,
                    is_active: isActive
                })
            });

            if (!response || !response.success) {
                const errorMessage = (response && response.message) || 'Bulk status update failed';
                throw new Error(errorMessage);
            }

            return response;

        } catch (error) {
            console.error('Bulk status update failed:', error);
            throw error;
        }
    }

    // Enterprise bulk delete books
    async bulkDeleteBooks(bookIds) {
        try {
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;

            if (!apiEndpoint || !apiRequest) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books/bulk-delete', {
                method: 'DELETE',
                body: JSON.stringify({
                    book_ids: bookIds
                })
            });

            if (!response || !response.success) {
                const errorMessage = (response && response.message) || 'Bulk delete failed';
                throw new Error(errorMessage);
            }

            return response;

        } catch (error) {
            console.error('Bulk delete failed:', error);
            throw error;
        }
    }

    // Enterprise edit modal opening
    async openEditModal(bookId) {
        try {
            if (!bookId) {
                throw new Error('Book ID is required');
            }

            // Get book details from backend
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;

            if (!apiEndpoint || !apiRequest) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books/' + bookId);
            // API call to book-service/handlers.rs line 234

            const responseData = (response && response.data) || null;
            if (!response || !response.success || !responseData) {
                throw new Error('Failed to load book details');
            }

            const book = responseData;

            // Populate edit form with book data
            this.populateEditForm(book);

            // Show modal
            const modal = document.getElementById('edit-book-modal');
            if (modal) {
                modal.classList.add('show');
                modal.style.display = 'flex';
            }

            this.editModal.isOpen = true;
            this.editModal.currentBook = book;

        } catch (error) {
            console.error('Failed to open edit modal:', error);
            const errorMessage = (error && error.message) || 'Unknown error';
            Utils.showNotification('Failed to load book details: ' + errorMessage, 'error');
        }
    }

    // Professional edit form population
    populateEditForm(book) {
        try {
            const bookData = book || {};

            // Basic book information
            const fields = {
                'edit-title': bookData.title || '',
                'edit-author': bookData.author || '',
                'edit-description': bookData.description || '',
                'edit-isbn': bookData.isbn || '',
                'edit-price': bookData.price || '',
                'edit-language': bookData.language || 'id',
                'edit-total-pages': bookData.total_pages || '',
                'edit-book-id': bookData.id || ''
            };

            // Populate form fields safely
            Object.entries(fields).forEach(([fieldId, value]) => {
                try {
                    const field = document.getElementById(fieldId);
                    if (field) {
                        field.value = value.toString();
                    }
                } catch (fieldError) {
                    console.warn('Failed to populate field', fieldId, ':', fieldError);
                }
            });

            // Set active status toggle
            const activeToggle = document.getElementById('edit-is-active');
            if (activeToggle) {
                activeToggle.checked = Boolean(bookData.is_active);
            }

            // Populate categories
            const categories = bookData.categories || [];
            this.populateEditCategories(categories);

        } catch (error) {
            console.error('Edit form population failed:', error);
            Utils.showNotification('Form loading error', 'warning');
        }
    }

    // Professional edit categories population
    populateEditCategories(bookCategories) {
        try {
            const categoriesContainer = document.getElementById('edit-categories');
            if (!categoriesContainer) return;

            const categoryList = Array.isArray(bookCategories) ? bookCategories : [];
            const bookCategoryIds = categoryList.map(cat => {
                const categoryData = cat || {};
                return (categoryData.id || cat || '').toString();
            });

            const availableCategories = this.state.categories || [];
            const categoryItems = availableCategories.map(category => {
                const categoryData = category || {};
                const categoryId = (categoryData.id || '').toString();
                const categoryName = categoryData.name || 'Unknown Category';
                const isChecked = bookCategoryIds.includes(categoryId);

                return `
                    <label class="category-item">
                        <input type="checkbox" 
                               name="edit_categories" 
                               value="${Utils.escapeHtml && Utils.escapeHtml(categoryId) || categoryId}" 
                               class="category-checkbox"
                               ${isChecked ? 'checked' : ''}>
                        <span class="category-label">${Utils.escapeHtml && Utils.escapeHtml(categoryName) || categoryName}</span>
                    </label>
                `;
            }).join('');

            categoriesContainer.innerHTML = categoryItems;

        } catch (error) {
            console.error('Edit categories population failed:', error);
        }
    }

    // Professional edit modal closing
    closeEditModal() {
        try {
            const modal = document.getElementById('edit-book-modal');
            if (modal) {
                modal.classList.remove('show');
                modal.style.display = 'none';
            }

            this.editModal.isOpen = false;
            this.editModal.currentBook = null;

            // Clear form errors
            this.clearFormErrors();

        } catch (error) {
            console.error('Edit modal close failed:', error);
        }
    }

    // Enterprise book update handling
    async handleBookUpdate() {
        try {
            if (!this.editModal.currentBook) {
                throw new Error('No book selected for editing');
            }

            // Get form data
            const formData = this.getEditFormData();

            // Validate form data
            const validation = this.validateEditForm(formData);
            if (!validation.isValid) {
                const errorMessage = (validation && validation.error) || 'Validation failed';
                Utils.showNotification(errorMessage, 'error');
                return;
            }

            Utils.showLoading && Utils.showLoading('Updating book...');

            // Update book via API
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;
            const bookId = this.editModal.currentBook.id;

            if (!apiEndpoint || !apiRequest || !bookId) {
                throw new Error('API client or book ID not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books/' + bookId, {
                method: 'PUT',
                body: JSON.stringify(formData)
            });

            if (!response || !response.success) {
                const errorMessage = (response && response.message) || 'Book update failed';
                throw new Error(errorMessage);
            }

            // Close modal
            this.closeEditModal();

            // Reload books
            await this.loadBooks();

            Utils.hideLoading && Utils.hideLoading();
            Utils.showNotification('Book updated successfully', 'success');

        } catch (error) {
            Utils.hideLoading && Utils.hideLoading();
            console.error('Book update failed:', error);
            const errorMessage = (error && error.message) || 'Unknown error';
            Utils.showNotification('Failed to update book: ' + errorMessage, 'error');
        }
    }

    // Enterprise edit form data collection
    getEditFormData() {
        try {
            // Get selected categories safely
            const categoryCheckboxes = document.querySelectorAll('input[name="edit_categories"]:checked');
            const selectedCategories = Array.from(categoryCheckboxes).map(checkbox => {
                return (checkbox && checkbox.value) || '';
            }).filter(Boolean);

            return {
                title: this.getElementValue('edit-title') || '',
                author: this.getElementValue('edit-author') || '',
                description: this.getElementValue('edit-description') || '',
                isbn: this.getElementValue('edit-isbn') || '',
                price: parseFloat(this.getElementValue('edit-price')) || 0,
                language: this.getElementValue('edit-language') || 'id',
                total_pages: parseInt(this.getElementValue('edit-total-pages')) || null,
                is_active: this.getElementChecked('edit-is-active') || false,
                categories: selectedCategories
            };

        } catch (error) {
            console.error('Edit form data collection failed:', error);
            throw new Error('Failed to collect form data');
        }
    }

    // Professional element value getter
    getElementValue(elementId) {
        try {
            const element = document.getElementById(elementId);
            const value = element && element.value;
            return (typeof value === 'string') ? value.trim() : '';
        } catch (error) {
            console.warn('Failed to get element value for:', elementId);
            return '';
        }
    }

    // Professional element checked getter
    getElementChecked(elementId) {
        try {
            const element = document.getElementById(elementId);
            return element && element.checked || false;
        } catch (error) {
            console.warn('Failed to get element checked for:', elementId);
            return false;
        }
    }

    // Enterprise edit form validation
    validateEditForm(formData) {
        try {
            const data = formData || {};

            // Required fields validation
            if (!data.title || !data.title.trim()) {
                return { isValid: false, error: 'Book title is required' };
            }

            if (!data.author || !data.author.trim()) {
                return { isValid: false, error: 'Author is required' };
            }

            if (!data.price || data.price < 1000) {
                return { isValid: false, error: 'Price must be at least IDR 1,000' };
            }

            if (data.description && data.description.length > 1000) {
                return { isValid: false, error: 'Description must be less than 1000 characters' };
            }

            return { isValid: true };

        } catch (error) {
            console.error('Form validation failed:', error);
            return { isValid: false, error: 'Validation error occurred' };
        }
    }

    // Professional delete modal opening
    async openDeleteModal(bookId) {
        try {
            const books = this.state.books || [];
            const book = books.find(b => (b && b.id) === bookId);

            if (!book) {
                Utils.showNotification('Book not found', 'error');
                return;
            }

            // Update modal content
            const deleteBookTitle = document.getElementById('delete-book-title');
            if (deleteBookTitle) {
                const title = book.title || 'Unknown Title';
                const author = book.author || 'Unknown Author';
                deleteBookTitle.textContent = '"' + title + '" by ' + author;
            }

            // Store book ID for deletion
            this.deleteModal = { bookId: bookId };

            // Show modal
            const modal = document.getElementById('delete-book-modal');
            if (modal) {
                modal.classList.add('show');
                modal.style.display = 'flex';
            }

        } catch (error) {
            console.error('Delete modal opening failed:', error);
            Utils.showNotification('Error opening delete dialog', 'error');
        }
    }

    // Professional delete modal closing
    closeDeleteModal() {
        try {
            const modal = document.getElementById('delete-book-modal');
            if (modal) {
                modal.classList.remove('show');
                modal.style.display = 'none';
            }

            this.deleteModal = null;

        } catch (error) {
            console.error('Delete modal close failed:', error);
        }
    }

    // Enterprise book deletion handling
    async handleBookDelete() {
        try {
            if (!this.deleteModal || !this.deleteModal.bookId) {
                throw new Error('No book selected for deletion');
            }

            Utils.showLoading && Utils.showLoading('Deleting book...');

            // Delete book via API
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;
            const bookId = this.deleteModal.bookId;

            if (!apiEndpoint || !apiRequest || !bookId) {
                throw new Error('API client or book ID not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books/' + bookId, {
                method: 'DELETE'
            });

            if (!response || !response.success) {
                const errorMessage = (response && response.message) || 'Book deletion failed';
                throw new Error(errorMessage);
            }

            // Close modal
            this.closeDeleteModal();

            // Reload books
            await this.loadBooks();

            Utils.hideLoading && Utils.hideLoading();
            Utils.showNotification('Book deleted successfully', 'success');

        } catch (error) {
            Utils.hideLoading && Utils.hideLoading();
            console.error('Book deletion failed:', error);
            const errorMessage = (error && error.message) || 'Unknown error';
            Utils.showNotification('Failed to delete book: ' + errorMessage, 'error');
        }
    }

    // Enterprise book status toggle
    async toggleBookStatus(bookId) {
        try {
            const books = this.state.books || [];
            const book = books.find(b => (b && b.id) === bookId);

            if (!book) {
                throw new Error('Book not found');
            }

            const newStatus = !book.is_active;
            const actionText = newStatus ? 'activate' : 'deactivate';
            const bookTitle = book.title || 'this book';

            if (!confirm('Are you sure you want to ' + actionText + ' "' + bookTitle + '"?')) {
                return;
            }

            Utils.showLoading && Utils.showLoading(actionText.charAt(0).toUpperCase() + actionText.slice(1) + 'ing book...');

            // Update status via API
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;

            if (!apiEndpoint || !apiRequest) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books/' + bookId + '/status', {
                method: 'PUT',
                body: JSON.stringify({
                    is_active: newStatus
                })
            });

            if (!response || !response.success) {
                const errorMessage = (response && response.message) || 'Status update failed';
                throw new Error(errorMessage);
            }

            // Update local state
            book.is_active = newStatus;

            // Re-render books
            this.renderBooks();

            Utils.hideLoading && Utils.hideLoading();
            Utils.showNotification('Book ' + actionText + 'd successfully', 'success');

        } catch (error) {
            Utils.hideLoading && Utils.hideLoading();
            console.error('Status toggle failed:', error);
            const errorMessage = (error && error.message) || 'Unknown error';
            Utils.showNotification('Failed to update book status: ' + errorMessage, 'error');
        }
    }

    // Professional store view
    viewBookInStore(bookId) {
        try {
            if (!bookId) {
                Utils.showNotification('Book ID not available', 'warning');
                return;
            }

            const storeUrl = '../book.html?id=' + encodeURIComponent(bookId);
            window.open(storeUrl, '_blank');

        } catch (error) {
            console.error('Store view failed:', error);
            Utils.showNotification('Failed to open book in store', 'error');
        }
    }

    // Enterprise book export
    async exportBooks() {
        try {
            if (!confirm('Export all books data to CSV?')) {
                return;
            }

            Utils.showLoading && Utils.showLoading('Preparing export...');

            // Get all books data for export
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiRequest = this.api && this.api.request;

            if (!apiEndpoint || !apiRequest) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(apiEndpoint + '/admin/books/export', {
                method: 'POST',
                body: JSON.stringify({
                    format: 'csv',
                    filters: this.state.filters || {}
                })
            });

            if (!response || !response.success) {
                const errorMessage = (response && response.message) || 'Export failed';
                throw new Error(errorMessage);
            }

            // Create download link
            const csvData = (response && response.data) || '';
            const blob = new Blob([csvData], { type: 'text/csv' });
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            const currentDate = new Date().toISOString().split('T')[0];

            a.href = url;
            a.download = 'books_export_' + currentDate + '.csv';
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            window.URL.revokeObjectURL(url);

            Utils.hideLoading && Utils.hideLoading();
            Utils.showNotification('Books exported successfully', 'success');

        } catch (error) {
            Utils.hideLoading && Utils.hideLoading();
            console.error('Export failed:', error);
            const errorMessage = (error && error.message) || 'Unknown error';
            Utils.showNotification('Export failed: ' + errorMessage, 'error');
        }
    }

    // Enterprise pagination update
    updatePagination() {
        try {
            const totalPages = Math.ceil((this.state.totalBooks || 0) / (this.state.itemsPerPage || 25));
            const paginationControls = document.getElementById('pagination-controls');

            if (!paginationControls || totalPages <= 1) {
                if (paginationControls) paginationControls.innerHTML = '';
                return;
            }

            // Generate pagination HTML
            let paginationHTML = '';
            const currentPage = this.state.currentPage || 1;

            // Previous button
            if (currentPage > 1) {
                paginationHTML += `
                    <button class="pagination-btn" data-page="${currentPage - 1}">
                        <i class="fas fa-chevron-left"></i> Previous
                    </button>
                `;
            }

            // Page numbers
            const startPage = Math.max(1, currentPage - 2);
            const endPage = Math.min(totalPages, currentPage + 2);

            if (startPage > 1) {
                paginationHTML += '<button class="pagination-btn" data-page="1">1</button>';
                if (startPage > 2) {
                    paginationHTML += '<span class="pagination-ellipsis">...</span>';
                }
            }

            for (let i = startPage; i <= endPage; i++) {
                const isActive = i === currentPage;
                paginationHTML += `
                    <button class="pagination-btn ${isActive ? 'active' : ''}" 
                            data-page="${i}">
                        ${i}
                    </button>
                `;
            }

            if (endPage < totalPages) {
                if (endPage < totalPages - 1) {
                    paginationHTML += '<span class="pagination-ellipsis">...</span>';
                }
                paginationHTML += '<button class="pagination-btn" data-page="' + totalPages + '">' + totalPages + '</button>';
            }

            // Next button
            if (currentPage < totalPages) {
                paginationHTML += `
                    <button class="pagination-btn" data-page="${currentPage + 1}">
                        Next <i class="fas fa-chevron-right"></i>
                    </button>
                `;
            }

            paginationControls.innerHTML = paginationHTML;

            // Setup pagination event listeners
            const paginationBtns = paginationControls.querySelectorAll('.pagination-btn');
            paginationBtns.forEach(btn => {
                btn.addEventListener('click', () => {
                    try {
                        const page = parseInt(btn.dataset && btn.dataset.page);
                        if (page && page !== currentPage) {
                            this.state.currentPage = page;
                            this.loadBooks();
                        }
                    } catch (error) {
                        console.error('Pagination click failed:', error);
                    }
                });
            });

        } catch (error) {
            console.error('Pagination update failed:', error);
        }
    }

    // Professional book count update
    updateBookCount() {
        try {
            const currentPage = this.state.currentPage || 1;
            const itemsPerPage = this.state.itemsPerPage || 25;
            const totalBooks = this.state.totalBooks || 0;
            const currentBooks = (this.state.books || []).length;

            const bookCount = document.getElementById('order-count') || document.getElementById('book-count');
            if (bookCount) {
                const startItem = (currentPage - 1) * itemsPerPage + 1;
                const endItem = Math.min(currentPage * itemsPerPage, totalBooks);

                bookCount.textContent = 'Showing ' + startItem + '-' + endItem + ' of ' + totalBooks + ' books';
            }

            // Update pagination info
            const paginationInfo = document.getElementById('pagination-info');
            if (paginationInfo) {
                paginationInfo.textContent = 'Showing ' + currentBooks + ' of ' + totalBooks + ' books';
            }

            // Update header stats
            const headerTotalBooks = document.getElementById('header-total-books');
            const headerActiveBooks = document.getElementById('header-active-books');

            if (headerTotalBooks) {
                headerTotalBooks.textContent = totalBooks.toLocaleString();
            }

            if (headerActiveBooks) {
                const books = this.state.books || [];
                const activeCount = books.filter(book => book && book.is_active).length;
                headerActiveBooks.textContent = activeCount.toLocaleString();
            }

        } catch (error) {
            console.error('Book count update failed:', error);
        }
    }

    // Enterprise book statistics update
    updateBookStatistics(stats) {
        try {
            if (!stats) return;

            const data = (stats && stats.data) || stats;
            if (!data) return;

            // Update stat cards safely
            const updates = {
                'total-books-stat': parseInt(data.total) || 0,
                'active-books-stat': parseInt(data.active) || 0,
                'total-downloads-stat': parseInt(data.total_downloads) || 0,
                'books-with-pdf-stat': parseInt(data.books_with_pdf) || 0
            };

            Object.entries(updates).forEach(([id, value]) => {
                try {
                    const element = document.getElementById(id);
                    if (element) {
                        element.textContent = value.toLocaleString();
                    }
                } catch (elementError) {
                    console.warn('Failed to update stat element:', id, elementError);
                }
            });

            // Update change indicators safely
            const totalBooks = parseInt(data.total) || 0;
            const booksWithPdf = parseInt(data.books_with_pdf) || 0;
            const monthlyChange = parseFloat(data.monthly_change) || 0;

            const changes = {
                'books-change': monthlyChange,
                'active-change': 'Available for sale',
                'downloads-change': 'All time downloads',
                'pdf-percentage': Math.round((booksWithPdf / Math.max(totalBooks, 1)) * 100) + '% complete'
            };

            Object.entries(changes).forEach(([id, value]) => {
                try {
                    const element = document.getElementById(id);
                    if (element) {
                        if (typeof value === 'string') {
                            element.innerHTML = '<i class="fas fa-check"></i> ' + value;
                        } else {
                            const icon = value >= 0 ? 'fa-arrow-up' : 'fa-arrow-down';
                            const className = value >= 0 ? 'positive' : 'negative';
                            element.className = 'stat-change ' + className;
                            element.innerHTML = '<i class="fas ' + icon + '"></i> ' + Math.abs(value).toFixed(1) + '% from last month';
                        }
                    }
                } catch (elementError) {
                    console.warn('Failed to update change element:', id, elementError);
                }
            });

        } catch (error) {
            console.error('Statistics update failed:', error);
        }
    }

    // Professional category filter population
    populateCategoryFilter(categories) {
        try {
            const categoryFilter = document.getElementById('category-filter');
            if (!categoryFilter) return;

            const categoryList = Array.isArray(categories) ? categories : [];

            // Clear existing options (except default)
            const defaultOption = categoryFilter.querySelector('option[value=""]');
            categoryFilter.innerHTML = '';

            if (defaultOption) {
                categoryFilter.appendChild(defaultOption);
            } else {
                const defaultOpt = document.createElement('option');
                defaultOpt.value = '';
                defaultOpt.textContent = 'All Categories';
                categoryFilter.appendChild(defaultOpt);
            }

            // Add category options
            categoryList.forEach(category => {
                try {
                    const categoryData = category || {};
                    const categoryId = (categoryData.id || '').toString();
                    const categoryName = categoryData.name || 'Unknown Category';

                    const option = document.createElement('option');
                    option.value = categoryId;
                    option.textContent = categoryName;
                    categoryFilter.appendChild(option);
                } catch (categoryError) {
                    console.warn('Failed to add category option:', categoryError);
                }
            });

        } catch (error) {
            console.error('Category filter population failed:', error);
        }
    }

    // Professional sort indicators update
    updateSortIndicators() {
        try {
            const sortableHeaders = document.querySelectorAll('.sortable');

            sortableHeaders.forEach(header => {
                try {
                    const icon = header.querySelector('i');
                    if (!icon) return;

                    const headerSort = header.dataset && header.dataset.sort;
                    const currentSort = this.state.sortBy;

                    if (headerSort === currentSort) {
                        // Active sort column
                        const sortOrder = this.state.sortOrder || 'asc';
                        icon.className = sortOrder === 'asc' ? 'fas fa-sort-up' : 'fas fa-sort-down';
                        header.classList.add('sort-active');
                    } else {
                        // Inactive sort column
                        icon.className = 'fas fa-sort';
                        header.classList.remove('sort-active');
                    }
                } catch (headerError) {
                    console.warn('Failed to update sort indicator:', headerError);
                }
            });

        } catch (error) {
            console.error('Sort indicators update failed:', error);
        }
    }

    // Professional loading state management
    showLoadingState() {
        try {
            const loadingBooks = document.getElementById('loading-books');
            const emptyBooks = document.getElementById('empty-books');

            if (loadingBooks) loadingBooks.style.display = 'block';
            if (emptyBooks) emptyBooks.style.display = 'none';

        } catch (error) {
            console.error('Show loading state failed:', error);
        }
    }

    hideLoadingState() {
        try {
            const loadingBooks = document.getElementById('loading-books');
            if (loadingBooks) loadingBooks.style.display = 'none';

        } catch (error) {
            console.error('Hide loading state failed:', error);
        }
    }

    // Professional empty state management
    showEmptyState() {
        try {
            const emptyBooks = document.getElementById('empty-books');
            const loadingBooks = document.getElementById('loading-books');

            if (emptyBooks) emptyBooks.style.display = 'block';
            if (loadingBooks) loadingBooks.style.display = 'none';

            // Hide table/grid containers and clear content
            const tableContainer = document.getElementById('table-view-container');
            const gridContainer = document.getElementById('grid-view-container');

            if (tableContainer) {
                const tableBody = document.getElementById('books-table-body');
                if (tableBody) tableBody.innerHTML = '';
            }

            if (gridContainer) {
                const grid = document.getElementById('books-grid');
                if (grid) grid.innerHTML = '';
            }

        } catch (error) {
            console.error('Show empty state failed:', error);
        }
    }

    // Enterprise form errors clearing
    clearFormErrors() {
        try {
            const errorElements = document.querySelectorAll('.form-error');
            const inputElements = document.querySelectorAll('.form-input');

            errorElements.forEach(error => {
                try {
                    error.textContent = '';
                    error.style.display = 'none';
                } catch (errorElementError) {
                    console.warn('Failed to clear error element:', errorElementError);
                }
            });

            inputElements.forEach(input => {
                try {
                    input.style.borderColor = '';
                } catch (inputElementError) {
                    console.warn('Failed to clear input style:', inputElementError);
                }
            });

        } catch (error) {
            console.error('Form errors clearing failed:', error);
        }
    }

    // Professional real-time updates setup
    setupRealTimeUpdates() {
        try {
            // Setup real-time updates for book statistics
            const apiSetupRealTime = this.api && this.api.setupRealTimeUpdates;

            if (!apiSetupRealTime) {
                console.warn('Real-time updates not available');
                return;
            }

            this.realTimeUpdates = apiSetupRealTime((eventType, data) => {
                try {
                    switch (eventType) {
                        case 'book_updated':
                            this.handleRealTimeBookUpdate(data);
                            break;
                        case 'book_deleted':
                            this.handleRealTimeBookDeleted(data);
                            break;
                        default:
                            console.log('Real-time event:', eventType, data);
                    }
                } catch (eventError) {
                    console.error('Real-time event handling failed:', eventError);
                }
            });

        } catch (error) {
            console.warn('Real-time updates setup failed:', error);
        }
    }

    // Professional real-time book update handling
    handleRealTimeBookUpdate(data) {
        try {
            if (!data || !data.book_id) return;

            // Find and update book in current list
            const books = this.state.books || [];
            const bookIndex = books.findIndex(book => (book && book.id) === data.book_id);

            if (bookIndex !== -1) {
                // Update book data safely
                const currentBook = books[bookIndex] || {};
                const updatedBook = Object.assign({}, currentBook, data);
                this.state.books[bookIndex] = updatedBook;

                // Re-render books
                this.renderBooks();
            }

        } catch (error) {
            console.error('Real-time book update handling failed:', error);
        }
    }

    // Professional real-time book deletion handling
    handleRealTimeBookDeleted(data) {
        try {
            if (!data || !data.book_id) return;

            // Remove book from current list
            const books = this.state.books || [];
            this.state.books = books.filter(book => (book && book.id) !== data.book_id);

            // Update total count
            this.state.totalBooks = Math.max(0, (this.state.totalBooks || 0) - 1);

            // Re-render books and update count
            this.renderBooks();
            this.updateBookCount();

        } catch (error) {
            console.error('Real-time book deletion handling failed:', error);
        }
    }

    // Professional admin profile update
    updateAdminProfile(adminUser) {
        try {
            const adminNameEl = document.getElementById('admin-name');
            if (adminNameEl && adminUser) {
                const fullName = (adminUser && adminUser.full_name) || 'Admin';
                adminNameEl.textContent = fullName;
            }
        } catch (error) {
            console.error('Admin profile update failed:', error);
        }
    }

    // Enterprise logout handling
    async handleLogout() {
        try {
            if (!confirm('Are you sure you want to logout?')) {
                return;
            }

            // Cleanup resources before logout
            this.cleanup();

            const authHandler = this.auth && this.auth.handleLogout;
            if (authHandler) {
                await authHandler();
            } else {
                // Fallback logout
                window.location.href = 'index.html';
            }

        } catch (error) {
            console.error('Logout failed:', error);
            Utils.showNotification('Logout failed', 'error');
        }
    }

    // Enterprise resource cleanup
    cleanup() {
        try {
            // Clear real-time updates
            if (this.realTimeUpdates) {
                try {
                    this.realTimeUpdates();
                } catch (cleanupError) {
                    console.warn('Real-time updates cleanup failed:', cleanupError);
                }
            }

            // Clear state safely
            this.state.books = [];
            this.state.selectedBooks.clear();
            this.state.error = null;

            // Clear modal states
            this.editModal.isOpen = false;
            this.editModal.currentBook = null;
            this.deleteModal = null;

            console.log('Book manager cleanup completed');

        } catch (error) {
            console.warn('Cleanup failed:', error);
        }
    }
}

// Enterprise export with error handling
try {
    window.BookManager = BookManager;
} catch (error) {
    console.error('Failed to export BookManager:', error);
}