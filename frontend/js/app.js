// /pdf-bookstore/frontend/js/app.js

// Main Bookstore Application class untuk handle UI updates dan user interactions
class BookstoreApp {
    constructor() {
        this.api = new BookstoreAPI(); // API client instance dari api.js
        this.auth = new AuthManager(this.api); // Auth manager dari auth.js
        this.cart = new CartManager(this.api, this.auth); // Cart manager dari cart.js
        this.payment = new PaymentManager(this.api, this.auth); // Payment manager dari payment.js

        this.currentUser = null;
        this.books = [];
        this.categories = [];
        this.currentPage = 1;
        this.totalPages = 1;
        this.isLoading = false;

        // Configuration untuk pagination dan display
        this.config = {
            booksPerPage: 12,
            searchDebounceTime: 300,
            loadingTimeout: 30000
        };

        // Initialize application
        this.init();
    }

    // Function untuk initialize application dengan auth check dan event setup
    async init() {
        try {
            Utils.showLoading('Initializing application...');

            // Setup global error handler
            this.setupGlobalErrorHandler();

            // Check authentication status
            await this.checkAuthenticationStatus();

            // Setup event listeners untuk UI interactions
            this.setupEventListeners();

            // Load initial data (categories dan books)
            await this.loadInitialData();

            // Setup payment callbacks
            this.setupPaymentCallbacks();

            // Setup cart callbacks
            this.setupCartCallbacks();

            // Update UI dengan current state
            this.updateAuthUI();
            this.updateCartUI();

            Utils.hideLoading();

        } catch (error) {
            console.error('App initialization failed:', error);
            Utils.hideLoading();
            Utils.showNotification('Failed to initialize application. Please refresh the page.', 'error');
        }
    }

    // Function untuk check authentication status saat app start
    async checkAuthenticationStatus() {
        try {
            // Check stored auth dengan auth manager
            const authStatus = await this.auth.checkAuthStatus();

            if (authStatus.isAuthenticated) {
                this.currentUser = authStatus.user;
                console.log('User authenticated:', this.currentUser);
            } else {
                this.currentUser = null;
                console.log('User not authenticated');
            }
        } catch (error) {
            console.warn('Auth status check failed:', error);
            this.currentUser = null;
        }
    }

    // Function untuk load initial data (categories dan books)
    async loadInitialData() {
        try {
            // Load categories untuk filter dropdown
            await this.loadCategories();

            // Load initial books dengan default parameters
            await this.loadBooks();

        } catch (error) {
            console.error('Failed to load initial data:', error);
            Utils.showNotification('Failed to load content. Please try again.', 'error');
        }
    }

    // Function untuk load categories dari book service
    async loadCategories() {
        try {
            const response = await this.api.getCategories();

            if (response.success && response.data) {
                this.categories = response.data;
                this.renderCategoryFilter();
            }
        } catch (error) {
            console.warn('Failed to load categories:', error);
        }
    }

    // Function untuk load books dengan search, filter, dan pagination
    async loadBooks(params = {}, append = false) {
        if (this.isLoading) return; // Prevent concurrent loads

        try {
            this.isLoading = true;

            if (!append) {
                Utils.showLoading('Loading books...');
            }

            // Build query parameters sesuai BookQueryParams dari backend
            const queryParams = {
                page: params.page || this.currentPage,
                limit: params.limit || this.config.booksPerPage,
                search: params.search || '',
                category: params.category || '',
                author: params.author || '',
                language: params.language || '',
                min_price: params.min_price || '',
                max_price: params.max_price || '',
                sort_by: params.sort_by || 'created_at',
                sort_order: params.sort_order || 'desc'
            };

            // Remove empty parameters
            Object.keys(queryParams).forEach(key => {
                if (queryParams[key] === '' || queryParams[key] === null || queryParams[key] === undefined) {
                    delete queryParams[key];
                }
            });

            const response = await this.api.getBooks(queryParams);

            if (response.success && response.data) {
                if (append) {
                    this.books = [...this.books, ...response.data];
                } else {
                    this.books = response.data;
                }

                // Update pagination info dari PaginationMeta
                if (response.pagination) {
                    this.currentPage = response.pagination.current_page;
                    this.totalPages = response.pagination.total_pages;
                    this.updatePaginationUI(response.pagination);
                }

                this.renderBooks();
            } else {
                throw new Error(response.message || 'Failed to load books');
            }

        } catch (error) {
            console.error('Failed to load books:', error);
            Utils.showNotification('Failed to load books. Please try again.', 'error');
        } finally {
            this.isLoading = false;
            Utils.hideLoading();
        }
    }

    // Function untuk setup all event listeners
    setupEventListeners() {
        // Authentication events
        this.setupAuthEventListeners();

        // Book browsing events
        this.setupBookEventListeners();

        // Search dan filter events
        this.setupSearchEventListeners();

        // Cart events
        this.setupCartEventListeners();

        // Admin events
        this.setupAdminEventListeners();
    }

    // Function untuk setup authentication event listeners
    setupAuthEventListeners() {
        // Login form submission
        const loginForm = document.getElementById('login-form');
        if (loginForm) {
            loginForm.addEventListener('submit', this.handleLogin.bind(this));
        }

        // Register form submission
        const registerForm = document.getElementById('register-form');
        if (registerForm) {
            registerForm.addEventListener('submit', this.handleRegister.bind(this));
        }

        // Logout button click
        const logoutBtn = document.getElementById('logout-btn');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', this.handleLogout.bind(this));
        }

        // Auth state changes dari AuthManager
        this.auth.onAuthChange((eventType, userData) => {
            if (eventType === 'login') {
                this.currentUser = userData;
                this.updateAuthUI();
                Utils.showNotification('Login successful!', 'success');
            } else if (eventType === 'logout') {
                this.currentUser = null;
                this.updateAuthUI();
                Utils.showNotification('Logout successful', 'success');
            }
        });
    }

    // Function untuk setup book browsing event listeners
    setupBookEventListeners() {
        // Event delegation untuk book actions
        document.addEventListener('click', (e) => {
            // Book purchase button
            if (e.target.matches('.btn-purchase') || e.target.closest('.btn-purchase')) {
                const button = e.target.matches('.btn-purchase') ? e.target : e.target.closest('.btn-purchase');
                const bookId = button.dataset.bookId;
                if (bookId) {
                    this.handleBookPurchase(bookId);
                }
            }

            // Book download button
            if (e.target.matches('.btn-download') || e.target.closest('.btn-download')) {
                const button = e.target.matches('.btn-download') ? e.target : e.target.closest('.btn-download');
                const bookId = button.dataset.bookId;
                if (bookId) {
                    this.handleBookDownload(bookId);
                }
            }

            // Add to cart button
            if (e.target.matches('.btn-add-cart') || e.target.closest('.btn-add-cart')) {
                const button = e.target.matches('.btn-add-cart') ? e.target : e.target.closest('.btn-add-cart');
                const bookId = button.dataset.bookId;
                if (bookId) {
                    this.handleAddToCart(bookId);
                }
            }

            // Book detail view
            if (e.target.matches('.book-detail-link') || e.target.closest('.book-detail-link')) {
                const link = e.target.matches('.book-detail-link') ? e.target : e.target.closest('.book-detail-link');
                const bookId = link.dataset.bookId;
                if (bookId) {
                    this.handleBookDetail(bookId);
                }
            }
        });

        // Pagination events
        const loadMoreBtn = document.getElementById('load-more-btn');
        if (loadMoreBtn) {
            loadMoreBtn.addEventListener('click', this.loadMoreBooks.bind(this));
        }
    }

    // Function untuk setup search dan filter event listeners
    setupSearchEventListeners() {
        // Search input dengan debouncing
        const searchInput = document.getElementById('search-input');
        if (searchInput) {
            searchInput.addEventListener('input',
                Utils.debounce(this.handleSearch.bind(this), this.config.searchDebounceTime)
            );
        }

        // Category filter dropdown
        const categoryFilter = document.getElementById('category-filter');
        if (categoryFilter) {
            categoryFilter.addEventListener('change', this.handleCategoryFilter.bind(this));
        }

        // Sort dropdown
        const sortFilter = document.getElementById('sort-filter');
        if (sortFilter) {
            sortFilter.addEventListener('change', this.handleSortChange.bind(this));
        }

        // Price range filter
        const priceMinInput = document.getElementById('price-min');
        const priceMaxInput = document.getElementById('price-max');

        if (priceMinInput && priceMaxInput) {
            const handlePriceFilter = Utils.debounce(this.handlePriceFilter.bind(this), 500);
            priceMinInput.addEventListener('input', handlePriceFilter);
            priceMaxInput.addEventListener('input', handlePriceFilter);
        }
    }

    // Function untuk handle login form submission
    async handleLogin(event) {
        event.preventDefault();
        const formData = new FormData(event.target);

        try {
            Utils.showLoading('Logging in...');

            const result = await this.auth.login(
                formData.get('email'),
                formData.get('password'),
                formData.get('remember_me') === 'on'
            );

            if (result.success) {
                // Redirect atau update page
                const redirectUrl = new URLSearchParams(window.location.search).get('redirect') || '/';
                window.location.href = redirectUrl;
            }

        } catch (error) {
            Utils.showNotification(error.message, 'error');
        } finally {
            Utils.hideLoading();
        }
    }

    // Function untuk handle register form submission  
    async handleRegister(event) {
        event.preventDefault();
        const formData = new FormData(event.target);

        // Validate password confirmation
        const password = formData.get('password');
        const confirmPassword = formData.get('confirm_password');

        if (password !== confirmPassword) {
            Utils.showNotification('Passwords do not match', 'error');
            return;
        }

        try {
            Utils.showLoading('Creating account...');

            const userData = {
                email: formData.get('email'),
                password: password,
                full_name: formData.get('full_name')
            };

            const result = await this.auth.register(userData);

            if (result.success) {
                Utils.showNotification('Account created successfully!', 'success');
                window.location.href = '/';
            }

        } catch (error) {
            Utils.showNotification(error.message, 'error');
        } finally {
            Utils.hideLoading();
        }
    }

    // Function untuk handle logout
    async handleLogout() {
        try {
            await this.auth.logout();
            // Clear cart dan other state
            this.cart.clearCart();
            this.payment.cleanup();
            // Redirect ke home page
            window.location.href = '/';
        } catch (error) {
            Utils.showNotification('Logout failed. Please try again.', 'error');
        }
    }

    // Function untuk handle book purchase dengan payment flow
    async handleBookPurchase(bookId) {
        // Check authentication
        if (!this.auth.isAuthenticated()) {
            Utils.showNotification('Please log in to purchase books', 'warning');
            window.location.href = `/login.html?redirect=${encodeURIComponent(window.location.pathname)}`;
            return;
        }

        try {
            let purchaseStatus;
            try {
                purchaseStatus = await this.api.checkPurchaseStatus(bookId);
            } catch (error) {
                console.warn('Could not check purchase status:', error);
                // Continue with purchase flow if status check fails
                purchaseStatus = { has_purchased: false };
            }

            if (purchaseStatus && purchaseStatus.has_purchased) {
                Utils.showNotification('You already own this book!', 'info');
                return;
            }

            this.showPaymentMethodModal(bookId);

        } catch (error) {
            console.error('Purchase check failed:', error);
            Utils.showNotification('Failed to check purchase status. Please try again.', 'error');
        }
    }

    // Function untuk handle book download
    async handleBookDownload(bookId) {
        try {
            Utils.showLoading('Preparing download...');

            const blob = await this.api.downloadBook(bookId);

            // Create download link
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `book-${bookId}.pdf`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            window.URL.revokeObjectURL(url);

            Utils.showNotification('Download started!', 'success');

        } catch (error) {
            Utils.showNotification('Download failed: ' + error.message, 'error');
        } finally {
            Utils.hideLoading();
        }
    }

    // Function untuk handle add to cart
    async handleAddToCart(bookId) {
        try {
            const result = await this.cart.addToCart(bookId);
            if (result.success) {
                Utils.showNotification(result.message, 'success');
            }
        } catch (error) {
            Utils.showNotification(error.message, 'error');
        }
    }

    // Function untuk handle search input
    async handleSearch(event) {
        const query = event.target.value.trim();

        // Reset to first page untuk new search
        this.currentPage = 1;

        await this.loadBooks({
            search: query,
            page: 1
        });
    }

    // Function untuk setup payment callbacks
    setupPaymentCallbacks() {
        this.payment.onPaymentEvent((eventType, order, additionalData) => {
            switch (eventType) {
                case 'payment_success':
                    Utils.showNotification('Payment successful! You can now download the book.', 'success');
                    this.loadBooks(); // Refresh untuk update purchase status
                    break;

                case 'payment_failed':
                    Utils.showNotification(`Payment failed: ${additionalData}`, 'error');
                    break;

                case 'payment_cancelled':
                    Utils.showNotification('Payment was cancelled', 'warning');
                    break;

                case 'payment_pending':
                    Utils.showNotification('Payment is being processed...', 'info');
                    break;
            }
        });
    }

    // Function untuk setup cart callbacks
    setupCartCallbacks() {
        this.cart.onCartChange((cartItems, total) => {
            this.updateCartUI();
        });
    }

    // Function untuk render books list
    renderBooks() {
            const container = document.getElementById('books-container');
            if (!container) return;

            if (this.books.length === 0) {
                container.innerHTML = `
                <div class="no-books-message">
                    <h3>No books found</h3>
                    <p>Try adjusting your search or filter criteria.</p>
                </div>
            `;
                return;
            }

            container.innerHTML = this.books.map(bookData => {
                        const book = bookData.book || bookData; // Handle BookWithCategories structure
                        const categories = bookData.categories || [];

                        return `
                <div class="book-card" data-book-id="${book.id}">
                    <div class="book-cover">
                        <img src="${book.cover_path || '/assets/default-cover.jpg'}" 
                             alt="${Utils.escapeHtml(book.title)}" 
                             loading="lazy">
                    </div>
                    <div class="book-info">
                        <h3 class="book-title">${Utils.escapeHtml(book.title)}</h3>
                        <p class="book-author">by ${Utils.escapeHtml(book.author)}</p>
                        <p class="book-description">${Utils.truncateText(book.description || '', 100)}</p>
                        <div class="book-categories">
                            ${categories.map(cat => `<span class="category-tag">${Utils.escapeHtml(cat.name)}</span>`).join('')}
                        </div>
                        <div class="book-price">${Utils.formatCurrency(parseFloat(book.price))}</div>
                        <div class="book-actions">
                            ${this.renderBookActions(book)}
                        </div>
                    </div>
                </div>
            `;
        }).join('');
    }

    // Function untuk render book actions berdasarkan purchase status dan auth
    renderBookActions(book) {
        if (!this.auth.isAuthenticated()) {
            return `
                <button class="btn btn-primary btn-purchase" data-book-id="${book.id}">
                    Buy Now - ${Utils.formatCurrency(parseFloat(book.price))}
                </button>
                <a href="/book.html?id=${book.id}" class="btn btn-outline book-detail-link" data-book-id="${book.id}">
                    View Details
                </a>
            `;
        }

        // Check if user owns this book (this would need to be tracked in app state)
        const userOwnsBook = this.checkUserOwnsBook(book.id);
        
        if (userOwnsBook) {
            return `
                <button class="btn btn-success btn-download" data-book-id="${book.id}">
                    <i class="icon-download"></i> Download PDF
                </button>
                <a href="/reader.html?id=${book.id}" class="btn btn-outline">
                    <i class="icon-read"></i> Read Online
                </a>
            `;
        } else {
            return `
                <button class="btn btn-primary btn-purchase" data-book-id="${book.id}">
                    Buy Now - ${Utils.formatCurrency(parseFloat(book.price))}
                </button>
                <button class="btn btn-outline btn-add-cart" data-book-id="${book.id}">
                    <i class="icon-cart"></i> Add to Cart
                </button>
                <a href="/book.html?id=${book.id}" class="btn btn-outline book-detail-link" data-book-id="${book.id}">
                    View Details
                </a>
            `;
        }
    }

    // Function untuk check if user owns book (simplified - should be enhanced)
    checkUserOwnsBook(bookId) {
        // This is a placeholder - in real implementation, this would check against
        // user's purchase history or be tracked in application state
        return false;
    }

    // Function untuk render category filter dropdown
    renderCategoryFilter() {
        const categoryFilter = document.getElementById('category-filter');
        if (!categoryFilter) return;

        categoryFilter.innerHTML = `
            <option value="">All Categories</option>
            ${this.categories.map(category => `
                <option value="${category.slug}">${Utils.escapeHtml(category.name)}</option>
            `).join('')}
        `;
    }

    // Function untuk update authentication UI
    updateAuthUI() {
        const authElements = document.querySelectorAll('[data-auth]');
        const guestElements = document.querySelectorAll('[data-guest]');
        const userNameElements = document.querySelectorAll('.user-name');
        const userEmailElements = document.querySelectorAll('.user-email');

        if (this.currentUser) {
            // Show authenticated elements
            authElements.forEach(el => el.style.display = 'block');
            guestElements.forEach(el => el.style.display = 'none');

            // Update user info
            userNameElements.forEach(el => {
                el.textContent = this.currentUser.full_name || 'User';
            });
            userEmailElements.forEach(el => {
                el.textContent = this.currentUser.email || '';
            });
        } else {
            // Show guest elements
            authElements.forEach(el => el.style.display = 'none');
            guestElements.forEach(el => el.style.display = 'block');
        }
    }

    // Function untuk update cart UI
    updateCartUI() {
        const cartSummary = this.cart.getCartSummary();
        
        // Update cart badge
        const cartBadge = document.getElementById('cart-badge');
        if (cartBadge) {
            cartBadge.textContent = cartSummary.itemCount;
            cartBadge.style.display = cartSummary.itemCount > 0 ? 'inline' : 'none';
        }

        // Update cart total
        const cartTotal = document.getElementById('cart-total');
        if (cartTotal) {
            cartTotal.textContent = Utils.formatCurrency(cartSummary.total);
        }

        // Update cart items list
        const cartItemsList = document.getElementById('cart-items-list');
        if (cartItemsList) {
            if (cartSummary.isEmpty) {
                cartItemsList.innerHTML = '<p class="empty-cart">Your cart is empty</p>';
            } else {
                cartItemsList.innerHTML = cartSummary.items.map(item => `
                    <div class="cart-item" data-book-id="${item.bookId}">
                        <img src="${item.coverPath || '/assets/default-cover.jpg'}" alt="${item.title}" class="cart-item-cover">
                        <div class="cart-item-info">
                            <h4>${Utils.escapeHtml(item.title)}</h4>
                            <p>by ${Utils.escapeHtml(item.author)}</p>
                            <p class="cart-item-price">${Utils.formatCurrency(item.price)}</p>
                        </div>
                        <button class="btn btn-sm btn-danger remove-from-cart" data-book-id="${item.bookId}">
                            Remove
                        </button>
                    </div>
                `).join('');
            }
        }
    }

    // Function untuk update pagination UI
    updatePaginationUI(pagination) {
        const paginationContainer = document.getElementById('pagination-container');
        if (!paginationContainer) return;

        const { current_page, total_pages, has_prev, has_next } = pagination;

        paginationContainer.innerHTML = `
            <div class="pagination">
                <button class="btn btn-outline" ${!has_prev ? 'disabled' : ''} 
                        onclick="bookstoreApp.goToPage(${current_page - 1})">
                    Previous
                </button>
                <span class="pagination-info">
                    Page ${current_page} of ${total_pages}
                </span>
                <button class="btn btn-outline" ${!has_next ? 'disabled' : ''} 
                        onclick="bookstoreApp.goToPage(${current_page + 1})">
                    Next
                </button>
            </div>
        `;
    }

    // Function untuk navigate to specific page
    async goToPage(page) {
        if (page < 1 || page > this.totalPages || page === this.currentPage) {
            return;
        }

        this.currentPage = page;
        await this.loadBooks({ page });
    }

    // Function untuk load more books (infinite scroll)
    async loadMoreBooks() {
        if (this.currentPage >= this.totalPages) {
            return;
        }

        await this.loadBooks({ page: this.currentPage + 1 }, true);
    }

    // Function untuk show payment method selection modal
    showPaymentMethodModal(bookId) {
        const modal = document.createElement('div');
        modal.className = 'modal payment-modal';
        modal.innerHTML = `
            <div class="modal-content">
                <div class="modal-header">
                    <h3>Select Payment Method</h3>
                    <button class="modal-close">&times;</button>
                </div>
                <div class="modal-body">
                    <div class="payment-methods">
                        ${this.renderPaymentMethods()}
                    </div>
                </div>
                <div class="modal-footer">
                    <button class="btn btn-outline cancel-payment">Cancel</button>
                    <button class="btn btn-primary confirm-payment" disabled>
                        Proceed to Payment
                    </button>
                </div>
            </div>
        `;

        document.body.appendChild(modal);

        // Setup modal event listeners
        this.setupPaymentModalListeners(modal, bookId);

        // Show modal
        requestAnimationFrame(() => {
            modal.classList.add('show');
        });
    }

    // Function untuk render payment methods
    renderPaymentMethods() {
        const paymentMethods = this.payment.getPaymentMethods();
        
        return paymentMethods.map(method => `
            <div class="payment-method" data-method-id="${method.id}">
                <input type="radio" name="payment_method" value="${method.id}" id="payment_${method.id}">
                <label for="payment_${method.id}">
                    <div class="payment-method-icon">
                        <i class="${method.icon}"></i>
                    </div>
                    <div class="payment-method-info">
                        <h4>${method.name}</h4>
                        <p>${method.description}</p>
                        <small>Estimated time: ${method.estimatedTime}</small>
                    </div>
                </label>
            </div>
        `).join('');
    }

    // Function untuk setup payment modal event listeners
    setupPaymentModalListeners(modal, bookId) {
        const closeBtn = modal.querySelector('.modal-close');
        const cancelBtn = modal.querySelector('.cancel-payment');
        const confirmBtn = modal.querySelector('.confirm-payment');
        const paymentMethodInputs = modal.querySelectorAll('input[name="payment_method"]');

        // Close modal handlers
        [closeBtn, cancelBtn].forEach(btn => {
            btn.addEventListener('click', () => {
                modal.classList.remove('show');
                setTimeout(() => modal.remove(), 300);
            });
        });

        // Payment method selection
        paymentMethodInputs.forEach(input => {
            input.addEventListener('change', () => {
                confirmBtn.disabled = false;
            });
        });

        // Confirm payment
        confirmBtn.addEventListener('click', async () => {
            const selectedMethod = modal.querySelector('input[name="payment_method"]:checked');
            if (!selectedMethod) return;

            try {
                // Initiate payment process
                const result = await this.payment.initiatePayment(bookId, selectedMethod.value);
                
                if (result.success && result.paymentUrl) {
                    // Close modal
                    modal.classList.remove('show');
                    setTimeout(() => modal.remove(), 300);
                    
                    // Open payment window
                    await this.payment.openPaymentWindow(result.paymentUrl);
                }
            } catch (error) {
                Utils.showNotification(error.message, 'error');
            }
        });
    }

    // Function untuk setup global error handler
    setupGlobalErrorHandler() {
        window.addEventListener('error', (event) => {
            console.error('Global error:', event.error);
            Utils.showNotification('An unexpected error occurred. Please refresh the page.', 'error');
        });

        window.addEventListener('unhandledrejection', (event) => {
            console.error('Unhandled promise rejection:', event.reason);
            Utils.showNotification('An unexpected error occurred. Please try again.', 'error');
        });
    }

    // Function untuk handle category filter change
    async handleCategoryFilter(event) {
        const category = event.target.value;
        this.currentPage = 1;
        
        await this.loadBooks({
            category: category,
            page: 1
        });
    }

    // Function untuk handle sort change
    async handleSortChange(event) {
        const [sort_by, sort_order] = event.target.value.split('_');
        this.currentPage = 1;
        
        await this.loadBooks({
            sort_by: sort_by,
            sort_order: sort_order,
            page: 1
        });
    }

    // Function untuk handle price filter
    async handlePriceFilter() {
        const minPrice = document.getElementById('price-min')?.value;
        const maxPrice = document.getElementById('price-max')?.value;
        
        this.currentPage = 1;
        
        await this.loadBooks({
            min_price: minPrice,
            max_price: maxPrice,
            page: 1
        });
    }

    // Function untuk setup cart event listeners
    setupCartEventListeners() {
        // Remove from cart
        document.addEventListener('click', (e) => {
            if (e.target.matches('.remove-from-cart')) {
                const bookId = e.target.dataset.bookId;
                this.handleRemoveFromCart(bookId);
            }
        });

        // Checkout button
        const checkoutBtn = document.getElementById('checkout-btn');
        if (checkoutBtn) {
            checkoutBtn.addEventListener('click', this.handleCheckout.bind(this));
        }
    }

    // Function untuk handle remove from cart
    async handleRemoveFromCart(bookId) {
        try {
            const result = await this.cart.removeFromCart(bookId);
            if (result.success) {
                Utils.showNotification(result.message, 'success');
            }
        } catch (error) {
            Utils.showNotification(error.message, 'error');
        }
    }

    // Function untuk handle checkout process
    async handleCheckout() {
        if (!this.auth.isAuthenticated()) {
            Utils.showNotification('Please log in to checkout', 'warning');
            window.location.href = '/login.html';
            return;
        }

        try {
            const result = await this.cart.processCheckout();
            if (result.success && result.paymentUrl) {
                await this.payment.openPaymentWindow(result.paymentUrl);
            }
        } catch (error) {
            Utils.showNotification(error.message, 'error');
        }
    }

    // Function untuk setup admin event listeners
    setupAdminEventListeners() {
        // Only setup if user is admin
        if (this.currentUser && this.currentUser.role === 'admin') {
            // Admin book management events
            const adminPanel = document.getElementById('admin-panel');
            if (adminPanel) {
                this.setupAdminPanelEvents();
            }
        }
    }

    // Function untuk setup admin panel events
    setupAdminPanelEvents() {
        // Book upload form
        const bookUploadForm = document.getElementById('book-upload-form');
        if (bookUploadForm) {
            bookUploadForm.addEventListener('submit', this.handleBookUpload.bind(this));
        }

        // Delete book buttons
        document.addEventListener('click', (e) => {
            if (e.target.matches('.btn-delete-book')) {
                const bookId = e.target.dataset.bookId;
                this.handleDeleteBook(bookId);
            }
        });
    }

    // Function untuk handle book upload (admin only)
    async handleBookUpload(event) {
        event.preventDefault();
        
        if (!this.currentUser || this.currentUser.role !== 'admin') {
            Utils.showNotification('Admin access required', 'error');
            return;
        }

        try {
            Utils.showLoading('Uploading book...');
            
            const formData = new FormData(event.target);
            const result = await this.api.createBook(formData);
            
            if (result.success) {
                Utils.showNotification('Book uploaded successfully!', 'success');
                event.target.reset();
                await this.loadBooks(); // Refresh book list
            }
        } catch (error) {
            Utils.showNotification(error.message, 'error');
        } finally {
            Utils.hideLoading();
        }
    }

    // Function untuk handle delete book (admin only)
    async handleDeleteBook(bookId) {
        if (!this.currentUser || this.currentUser.role !== 'admin') {
            Utils.showNotification('Admin access required', 'error');
            return;
        }

        if (!confirm('Are you sure you want to delete this book?')) {
            return;
        }

        try {
            Utils.showLoading('Deleting book...');
            
            const result = await this.api.deleteBook(bookId);
            
            if (result.success) {
                Utils.showNotification('Book deleted successfully', 'success');
                await this.loadBooks(); // Refresh book list
            }
        } catch (error) {
            Utils.showNotification(error.message, 'error');
        } finally {
            Utils.hideLoading();
        }
    }

    // Function untuk cleanup application resources
    cleanup() {
        // Cleanup managers
        this.auth.cleanup();
        this.cart.cleanup && this.cart.cleanup();
        this.payment.cleanup();
    }
}

// Initialize app when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.bookstoreApp = new BookstoreApp();
});

// Export BookstoreApp class untuk global access
window.BookstoreApp = BookstoreApp;