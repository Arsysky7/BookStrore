// /pdf-bookstore/frontend/js/api.js

class BookstoreAPI {
    constructor() {
        this.baseURL = this.detectEnvironment();
        this.endpoints = {
            auth: `${this.baseURL.auth}/api/auth`,
            books: `${this.baseURL.books}/api/books`,
            payments: `${this.baseURL.payments}/api`,
            categories: `${this.baseURL.books}/api/categories`,
            upload: `${this.baseURL.books}/api/upload`
        };

        this.token = this.getStoredToken();
        this.timeout = 30000;

        this.defaultHeaders = {
            'Content-Type': 'application/json',
            'Accept': 'application/json',
        };
    }

    detectEnvironment() {
        const hostname = window.location.hostname;
        const isDevelopment = hostname === 'localhost' || hostname === '127.0.0.1';

        if (isDevelopment) {
            const gatewayUrl = 'http://localhost:8000';

            console.log('🔧 Development Mode - Using API Gateway');
            console.log('📡 Gateway URL:', gatewayUrl);
            console.log('✓ All requests will be routed through gateway');
            console.log('   - Auth requests: Gateway → Auth Service (3001)');
            console.log('   - Book requests: Gateway → Book Service (3002)');
            console.log('   - Payment requests: Gateway → Payment Service (3003)');
            return {
                auth: gatewayUrl,
                books: gatewayUrl,
                payments: gatewayUrl
            };
        } else {
            // Production: services are behind nginx proxy
            const baseUrl = `${window.location.protocol}//${window.location.host}`;
            return {
                auth: baseUrl,
                books: baseUrl,
                payments: baseUrl
            };
        }
    }

    async request(url, options = {}) {
        const config = {
            ...options,
            headers: {
                ...this.defaultHeaders,
                ...options.headers
            },
            signal: AbortSignal.timeout(this.timeout)
        };

        if (this.token && !options.skipAuth) {
            config.headers['Authorization'] = `Bearer ${this.token}`;
        }

        if (options.body instanceof FormData) {
            delete config.headers['Content-Type'];
        }

        try {
            const response = await fetch(url, config);

            if (!response.ok) {
                const errorData = await response.json().catch(() => ({
                    success: false,
                    message: `HTTP ${response.status}: ${response.statusText}`,
                    error_code: `HTTP_${response.status}`
                }));

                if (response.status === 401) {
                    this.clearAuth();
                    window.location.href = '/login.html?redirect=' + encodeURIComponent(window.location.pathname);
                }

                throw new Error(this.getErrorMessage(errorData));
            }

            const contentType = response.headers.get('content-type');
            if (contentType && contentType.includes('application/json')) {
                return await response.json();
            } else {
                return response;
            }

        } catch (error) {
            if (error.name === 'AbortError') {
                throw new Error('Request timeout. Please try again.');
            }

            console.error('API Request failed:', error);
            throw error;
        }
    }

    getErrorMessage(errorData) {
        if (typeof errorData === 'string') return errorData;
        if (errorData.message) return errorData.message;
        if (errorData.error) return errorData.error;
        if (errorData.detail) return errorData.detail;

        return 'An unexpected error occurred';
    }

    // ===== AUTHENTICATION METHODS =====

    async login(email, password) {
        const response = await this.request(`${this.endpoints.auth}/login`, {
            method: 'POST',
            body: JSON.stringify({ email, password }),
            skipAuth: true
        });

        // Handle login response and store token
        if (response.success && response.token) {
            this.setToken(response.token);
            return response;
        } else if (response.token) {
            // Backend might return token without success flag
            this.setToken(response.token);
            return { success: true, ...response };
        }

        throw new Error(response.message || 'Login failed');
    }

    async register(userData) {
        const response = await this.request(`${this.endpoints.auth}/register`, {
            method: 'POST',
            body: JSON.stringify(userData),
            skipAuth: true
        });

        if (response.success && response.token) {
            this.setToken(response.token);
        }

        return response;
    }

    async verifyToken() {
        try {
            const response = await this.request(`${this.endpoints.auth}/verify`);
            return response.success || response.valid || false;
        } catch (error) {
            this.clearAuth();
            return false;
        }
    }

    // ===== OAUTH METHODS =====

    async getOAuthStatus() {
        try {
            const response = await this.request(`${this.endpoints.auth}/oauth/status`, {
                skipAuth: true
            });
            return response;
        } catch (error) {
            console.error('Failed to get OAuth status:', error);
            return { social_login_enabled: false, google_oauth_enabled: false };
        }
    }

    async startGoogleOAuth(redirectUri = null) {
        try {
            const response = await this.request(`${this.endpoints.auth}/oauth/google`, {
                method: 'POST',
                body: JSON.stringify({
                    provider: 'google',
                    redirect_uri: redirectUri
                }),
                skipAuth: true
            });

            if (response.success && response.auth_url) {
                // Store state in sessionStorage for security validation
                if (response.state) {
                    sessionStorage.setItem('oauth_state', response.state);
                    sessionStorage.setItem('oauth_provider', 'google');
                    sessionStorage.setItem('oauth_timestamp', Date.now().toString());
                }
                return response;
            }

            throw new Error(response.message || 'Failed to start Google OAuth');
        } catch (error) {
            console.error('Google OAuth initialization failed:', error);
            throw error;
        }
    }

    handleOAuthCallback() {
        const urlParams = new URLSearchParams(window.location.search);
        const accessToken = urlParams.get('access_token');
        const refreshToken = urlParams.get('refresh_token');
        const sessionToken = urlParams.get('session_token');
        const loginSuccess = urlParams.get('login_success');
        const error = urlParams.get('error');

        if (error) {
            throw new Error(`OAuth error: ${error}`);
        }

        if (loginSuccess === 'true' && accessToken && refreshToken) {
            // Clear OAuth state
            sessionStorage.removeItem('oauth_state');
            sessionStorage.removeItem('oauth_provider');
            sessionStorage.removeItem('oauth_timestamp');

            // Store tokens
            this.setToken(accessToken);
            if (refreshToken) {
                localStorage.setItem('bookstore_refresh_token', refreshToken);
            }
            if (sessionToken) {
                localStorage.setItem('bookstore_session_token', sessionToken);
            }

            // Clear URL parameters
            window.history.replaceState({}, document.title, window.location.pathname);

            return {
                success: true,
                access_token: accessToken,
                refresh_token: refreshToken,
                session_token: sessionToken
            };
        }

        throw new Error('Invalid OAuth response');
    }

    validateOAuthState(expectedProvider = 'google') {
        const state = sessionStorage.getItem('oauth_state');
        const provider = sessionStorage.getItem('oauth_provider');
        const timestamp = sessionStorage.getItem('oauth_timestamp');

        // Check if OAuth state exists
        if (!state || !provider || !timestamp) {
            throw new Error('OAuth state not found');
        }

        // Check provider match
        if (provider !== expectedProvider) {
            throw new Error('OAuth provider mismatch');
        }

        // Check timestamp (state should be recent, within 10 minutes)
        const elapsed = Date.now() - parseInt(timestamp);
        if (elapsed > 10 * 60 * 1000) { // 10 minutes
            sessionStorage.removeItem('oauth_state');
            sessionStorage.removeItem('oauth_provider');
            sessionStorage.removeItem('oauth_timestamp');
            throw new Error('OAuth state expired');
        }

        return { state, provider };
    }

    clearOAuthState() {
        sessionStorage.removeItem('oauth_state');
        sessionStorage.removeItem('oauth_provider');
        sessionStorage.removeItem('oauth_timestamp');
    }

    async getProfile() {
        return this.request(`${this.endpoints.auth}/profile`);
    }

    logout() {
        this.token = null;
        localStorage.removeItem('auth_token');
        sessionStorage.clear();
    }

    isAuthenticated() {
        return !!this.token;
    }

    // ===== BOOK METHODS =====

    async getBooks(params = {}) {
        const queryString = new URLSearchParams();

        Object.entries(params).forEach(([key, value]) => {
            if (value !== null && value !== undefined && value !== '') {
                queryString.append(key, value.toString());
            }
        });

        const url = queryString.toString() ?
            `${this.endpoints.books}?${queryString.toString()}` :
            this.endpoints.books;

        return this.request(url, { skipAuth: true });
    }

    async getBook(id) {
        return this.request(`${this.endpoints.books}/${id}`, { skipAuth: true });
    }

    async downloadBook(bookId) {
        const response = await fetch(`${this.endpoints.books}/${bookId}/download`, {
            headers: {
                'Authorization': `Bearer ${this.token}`
            },
            signal: AbortSignal.timeout(this.timeout)
        });

        if (!response.ok) {
            const errorData = await response.json().catch(() => ({
                message: 'Download failed'
            }));
            throw new Error(errorData.message);
        }

        return response.blob();
    }

    async getCategories() {
        return this.request(this.endpoints.categories, { skipAuth: true });
    }

    async createOrder(bookId, paymentMethod = 'qris') {
        return this.request(`${this.endpoints.payments}/orders`, {
            method: 'POST',
            body: JSON.stringify({
                book_id: bookId,
                payment_method: paymentMethod
            })
        });
    }

    async getOrder(orderId) {
        return this.request(`${this.endpoints.payments}/orders/${orderId}`);
    }

    async getMyOrders(page = 1, limit = 10) {
        const params = new URLSearchParams({
            page: page.toString(),
            limit: limit.toString()
        });
        return this.request(`${this.endpoints.payments}/orders?${params}`);
    }

    async cancelOrder(orderId) {
        return this.request(`${this.endpoints.payments}/orders/${orderId}/cancel`, {
            method: 'PUT'
        });
    }

    async getLibraryBooks(params = {}) {
        const queryString = new URLSearchParams();
        Object.entries(params).forEach(([key, value]) => {
            if (value !== null && value !== undefined && value !== '') {
                queryString.append(key, value.toString());
            }
        });
        const url = queryString.toString() ?
            `${this.endpoints.books}/my-library?${queryString}` :
            `${this.endpoints.books}/my-library`;
        return this.request(url);
    }

    async getBookPreview(bookId) {
        return this.request(`${this.endpoints.books}/${bookId}/preview`, {
            skipAuth: true
        });
    }

    async getBookReviews(bookId) {
        return this.request(`${this.endpoints.books}/${bookId}/reviews`, {
            skipAuth: true
        });
    }

    async addBookReview(bookId, rating, comment) {
        return this.request(`${this.endpoints.books}/${bookId}/reviews`, {
            method: 'POST',
            body: JSON.stringify({ rating, comment })
        });
    }

    async getRelatedBooks(bookId) {
        return this.request(`${this.endpoints.books}/${bookId}/related`, {
            skipAuth: true
        });
    }

    async checkPurchaseStatus(bookId) {
        return this.request(`${this.endpoints.payments.replace('/api', '')}/api/purchases/${bookId}`);
    }

    async createBook(formData) {
        if (!(formData instanceof FormData)) {
            throw new Error('Book creation requires FormData for file uploads');
        }

        return this.request(`${this.endpoints.books}`, {
            method: 'POST',
            body: formData
        });
    }

    async updateBook(bookId, formData) {
        if (!(formData instanceof FormData)) {
            throw new Error('Book update requires FormData for file uploads');
        }

        return this.request(`${this.endpoints.books}/${bookId}`, {
            method: 'PUT',
            body: formData
        });
    }

    async deleteBook(bookId) {
        return this.request(`${this.endpoints.books}/${bookId}`, {
            method: 'DELETE'
        });
    }

    // ===== FILE UPLOAD METHODS =====

    async uploadPDF(formData) {
        return this.request(`${this.endpoints.upload}/pdf`, {
            method: 'POST',
            body: formData
        });
    }

    async uploadCover(formData) {
        return this.request(`${this.endpoints.upload}/cover`, {
            method: 'POST',
            body: formData
        });
    }

    // ===== UTILITY METHODS =====

    getAuthHeaders() {
        const headers = {...this.defaultHeaders };
        if (this.token) {
            headers['Authorization'] = `Bearer ${this.token}`;
        }
        return headers;
    }

    handleApiError(error) {
        const errorMessages = {
            'Failed to fetch': 'Network error. Please check your connection.',
            'VALIDATION_ERROR': 'Please check your input and try again.',
            'MISSING_TOKEN': 'Please log in to continue.',
            'INVALID_TOKEN': 'Your session has expired. Please log in again.',
            'INSUFFICIENT_PRIVILEGES': 'You do not have permission to perform this action.',
            'BOOK_NOT_FOUND': 'The requested book was not found.',
            'EMAIL_EXISTS': 'This email is already registered.',
            'BOOK_ALREADY_PURCHASED': 'You have already purchased this book.',
            'ORDER_NOT_FOUND': 'Order not found.',
            'PAYMENT_GATEWAY_ERROR': 'Payment processing failed. Please try again.',
            'FILE_TOO_LARGE': 'File is too large. Please choose a smaller file.',
            'INVALID_FILE_TYPE': 'Invalid file type. Please check allowed file formats.',
            'NETWORK_ERROR': 'Network connection error. Please try again.',
            'SERVER_ERROR': 'Server error. Please try again later.'
        };

        let errorKey = error.message;
        if (error.message && error.message.includes(':')) {
            errorKey = error.message.split(':')[0].trim();
        }

        return errorMessages[errorKey] || error.message || 'An unexpected error occurred';
    }

    formatFileSize(bytes) {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }

    formatCurrency(amount) {
        return new Intl.NumberFormat('id-ID', {
            style: 'currency',
            currency: 'IDR',
            minimumFractionDigits: 0,
            maximumFractionDigits: 0
        }).format(amount);
    }

    setTimeout(timeout) {
        this.timeout = timeout;
        return this;
    }

    async requestWithRetry(url, options = {}, maxRetries = 3) {
        let lastError;

        for (let attempt = 0; attempt < maxRetries; attempt++) {
            try {
                return await this.request(url, options);
            } catch (error) {
                lastError = error;

                if (error.message.includes('4') || attempt === maxRetries - 1) {
                    break;
                }

                const delay = Math.pow(2, attempt) * 1000;
                await new Promise(resolve => setTimeout(resolve, delay));
            }
        }

        throw lastError;
    }

    async batchRequest(requests, concurrency = 5) {
        const results = [];
        const errors = [];

        for (let i = 0; i < requests.length; i += concurrency) {
            const batch = requests.slice(i, i + concurrency);

            const batchPromises = batch.map(async(request, index) => {
                try {
                    const result = await this.request(request.url, request.options);
                    return { index: i + index, result, error: null };
                } catch (error) {
                    return { index: i + index, result: null, error };
                }
            });

            const batchResults = await Promise.all(batchPromises);

            batchResults.forEach(({ index, result, error }) => {
                if (error) {
                    errors.push({ index, error });
                } else {
                    results[index] = result;
                }
            });
        }

        return { results, errors };
    }

    // ===== TOKEN MANAGEMENT =====

    setToken(token) {
        this.token = token;
        localStorage.setItem('bookstore_token', token);
        localStorage.setItem('bookstore_token_timestamp', Date.now().toString());
    }

    getStoredToken() {
        const token = localStorage.getItem('bookstore_token');
        const timestamp = localStorage.getItem('bookstore_token_timestamp');

        if (token && timestamp) {
            const tokenAge = Date.now() - parseInt(timestamp);
            const maxAge = 24 * 60 * 60 * 1000; // 24 hours

            if (tokenAge > maxAge) {
                this.clearAuth();
                return null;
            }
        }

        return token;
    }

    clearAuth() {
        this.token = null;
        localStorage.removeItem('bookstore_token');
        localStorage.removeItem('bookstore_token_timestamp');
        localStorage.removeItem('bookstore_user');
    }

    // ===== AUTHENTICATION METHODS (EXTENDED) =====

    async verifyOTP(email, otp, rememberMe = false, deviceFingerprint = null) {
        const response = await this.request(`${this.endpoints.auth}/verify-otp`, {
            method: 'POST',
            body: JSON.stringify({
                email,
                otp,
                remember_me: rememberMe,
                device_fingerprint: deviceFingerprint
            }),
            skipAuth: true
        });

        if (response.success && response.token) {
            this.setToken(response.token);
            if (response.refresh_token) {
                localStorage.setItem('bookstore_refresh_token', response.refresh_token);
            }
        }

        return response;
    }

    async requestPasswordReset(email) {
        return this.request(`${this.endpoints.auth}/password-reset/request`, {
            method: 'POST',
            body: JSON.stringify({ email }),
            skipAuth: true
        });
    }

    async resetPassword(token, newPassword, confirmPassword) {
        return this.request(`${this.endpoints.auth}/password-reset/confirm`, {
            method: 'POST',
            body: JSON.stringify({
                token,
                new_password: newPassword,
                confirm_password: confirmPassword
            }),
            skipAuth: true
        });
    }

    async sendVerificationEmail() {
        return this.request(`${this.endpoints.auth}/email/send-verification`, {
            method: 'POST'
        });
    }

    async verifyEmail(token) {
        return this.request(`${this.endpoints.auth}/email/verify`, {
            method: 'POST',
            body: JSON.stringify({ token }),
            skipAuth: true
        });
    }

    async refreshAccessToken(refreshToken = null) {
        const token = refreshToken || localStorage.getItem('bookstore_refresh_token');

        if (!token) {
            throw new Error('No refresh token available');
        }

        const response = await this.request(`${this.endpoints.auth}/refresh`, {
            method: 'POST',
            body: JSON.stringify({
                refresh_token: token,
                device_fingerprint: this.getDeviceFingerprint()
            }),
            skipAuth: true
        });

        if (response.access_token) {
            this.setToken(response.access_token);
            if (response.refresh_token) {
                localStorage.setItem('bookstore_refresh_token', response.refresh_token);
            }
        }

        return response;
    }

    async logoutBackend(refreshToken = null, accessTokenJti = null) {
        const token = refreshToken || localStorage.getItem('bookstore_refresh_token');

        return this.request(`${this.endpoints.auth}/logout`, {
            method: 'POST',
            body: JSON.stringify({
                refresh_token: token,
                access_token_jti: accessTokenJti
            })
        });
    }

    async revokeAllTokens() {
        return this.request(`${this.endpoints.auth}/revoke-all`, {
            method: 'POST'
        });
    }

    async validateSession() {
        const sessionToken = localStorage.getItem('bookstore_session_token');

        return this.request(`${this.endpoints.auth}/session/validate`, {
            method: 'POST',
            headers: {
                'X-Session-Token': sessionToken
            }
        });
    }

    // ===== USER PROFILE METHODS =====

    async updateProfile(profileData) {
        return this.request(`${this.endpoints.auth}/profile`, {
            method: 'PUT',
            body: JSON.stringify(profileData)
        });
    }

    async changePassword(oldPassword, newPassword, confirmPassword) {
        return this.request(`${this.endpoints.auth}/password/change`, {
            method: 'POST',
            body: JSON.stringify({
                old_password: oldPassword,
                new_password: newPassword,
                confirm_password: confirmPassword
            })
        });
    }

    async getLoginHistory(limit = 20) {
        const params = new URLSearchParams({ limit: limit.toString() });
        return this.request(`${this.endpoints.auth}/login-history?${params}`);
    }

    async getMyActivity(limit = 20) {
        const params = new URLSearchParams({ limit: limit.toString() });
        return this.request(`${this.endpoints.auth}/my-activity?${params}`);
    }

    // ===== PAYMENT METHODS (EXTENDED) =====

    async requestRefund(orderId, reason = null, amount = null, bankAccount = null) {
        const payload = { reason };

        if (amount) payload.amount = amount;
        if (bankAccount) payload.bank_account = bankAccount;

        return this.request(`${this.endpoints.payments}/orders/${orderId}/refund`, {
            method: 'POST',
            body: JSON.stringify(payload)
        });
    }

    async getPaymentConfig() {
        return this.request(`${this.endpoints.payments}/payment/config`);
    }

    // ===== UTILITY METHODS =====

    getDeviceFingerprint() {
        const fingerprint = localStorage.getItem('device_fingerprint');

        if (fingerprint) {
            return fingerprint;
        }

        const newFingerprint = this.generateDeviceFingerprint();
        localStorage.setItem('device_fingerprint', newFingerprint);
        return newFingerprint;
    }

    generateDeviceFingerprint() {
        const navigator = window.navigator;
        const screen = window.screen;

        const components = [
            navigator.userAgent,
            navigator.language,
            screen.colorDepth,
            screen.width + 'x' + screen.height,
            new Date().getTimezoneOffset(),
            !!window.sessionStorage,
            !!window.localStorage
        ];

        const fingerprint = components.join('|');
        return btoa(fingerprint).substring(0, 32);
    }

    // ===== ADMIN METHODS =====

    async getAdminBookStats() {
        return this.request(`${this.endpoints.books.replace('/api/books', '/api/admin/books')}/stats`);
    }

    async getTopBooks(metric = 'downloads', limit = 10) {
        const params = new URLSearchParams({ metric, limit: limit.toString() });
        return this.request(`${this.endpoints.books.replace('/api/books', '/api/admin/books')}/top?${params}`);
    }

    async getSalesAnalytics(days = 30) {
        const params = new URLSearchParams({ days: days.toString() });
        return this.request(`${this.endpoints.books.replace('/api/books', '/api/admin/analytics')}/sales?${params}`);
    }

    async getPopularBooksChartData(limit = 10) {
        const params = new URLSearchParams({ limit: limit.toString() });
        return this.request(`${this.endpoints.books.replace('/api/books', '/api/admin/analytics')}/popular-books?${params}`);
    }

    async getCategoryAnalytics() {
        return this.request(`${this.endpoints.books.replace('/api/books', '/api/admin/analytics')}/categories`);
    }

    async getDashboardMetrics() {
        return this.request(`${this.endpoints.books.replace('/api/books', '/api/admin/books')}/dashboard-metrics`);
    }

    async getRecentActivity(limit = 20) {
        const params = new URLSearchParams({ limit: limit.toString() });
        return this.request(`${this.endpoints.books.replace('/api/books', '/api/admin/books')}/activity?${params}`);
    }

    // ===== ADMIN ORDER METHODS =====

    async getAdminOrderStats() {
        return this.request(`${this.endpoints.payments}/admin/orders/stats`);
    }

    async getRevenueAnalytics(period = 'monthly', days = 30) {
        const params = new URLSearchParams({
            period,
            days: days.toString()
        });
        return this.request(`${this.endpoints.payments}/admin/analytics/revenue?${params}`);
    }

    async getRecentOrders(limit = 10, status = null) {
        const params = new URLSearchParams({ limit: limit.toString() });
        if (status) params.append('status', status);
        return this.request(`${this.endpoints.payments}/admin/orders/recent?${params}`);
    }

    async updateOrderStatus(orderId, status, notes = null) {
        const payload = { status };
        if (notes) payload.notes = notes;

        return this.request(`${this.endpoints.payments}/admin/orders/${orderId}/status`, {
            method: 'PUT',
            body: JSON.stringify(payload)
        });
    }

    async getSystemHealth() {
        return this.request(`${this.endpoints.payments}/admin/system/health`);
    }

    async triggerMaintenance() {
        return this.request(`${this.endpoints.payments}/admin/maintenance/trigger`, {
            method: 'POST'
        });
    }

    async getSchedulerStatus() {
        return this.request(`${this.endpoints.payments}/admin/scheduler/status`);
    }

    // ===== ADMIN USER METHODS =====

    async getAdminUsers(page = 1, limit = 20) {
        const params = new URLSearchParams({
            page: page.toString(),
            limit: limit.toString()
        });
        return this.request(`${this.endpoints.auth}/admin/users?${params}`);
    }

    async getAdminUserStats() {
        return this.request(`${this.endpoints.auth}/admin/users/stats`);
    }

    async updateUserStatus(userId, isActive) {
        return this.request(`${this.endpoints.auth}/admin/users/${userId}/status`, {
            method: 'PUT',
            body: JSON.stringify({ is_active: isActive })
        });
    }
}

window.BookstoreAPI = BookstoreAPI;

window.api = new BookstoreAPI();