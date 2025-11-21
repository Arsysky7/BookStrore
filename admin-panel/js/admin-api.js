// /pdf-bookstore/admin-panel/js/admin-api.js

// EPIC AdminAPI class untuk complete admin panel backend integration
class AdminAPI {
    constructor() {
        // Service endpoints configuration sesuai dengan all backend services
        this.endpoints = {
            auth: window.API_BASE_URL ? `${window.API_BASE_URL}/auth` : 'http://localhost:3001/api',
            books: window.API_BASE_URL ? `${window.API_BASE_URL}/books` : 'http://localhost:3002/api',
            payments: window.API_BASE_URL ? `${window.API_BASE_URL}/payments` : 'http://localhost:3003/api'
        };

        // Get stored auth token untuk admin access
        this.token = localStorage.getItem('auth_token');

        // Request timeout configuration
        this.timeout = 30000;

        // Cache untuk performance optimization
        this.cache = new Map();
        this.cacheTimeout = 5 * 60 * 1000; // 5 minutes cache

        // Real-time updates management
        this.realTimeCallbacks = new Set();
        this.realTimeInterval = null;
    }

    // Generic HTTP request handler dengan admin authentication dan error handling
    async request(url, options = {}) {
        const config = {
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${this.token}`,
                ...options.headers
            },
            signal: AbortSignal.timeout(this.timeout),
            ...options
        };

        try {
            const response = await fetch(url, config);

            // Handle admin-specific auth errors
            if (response.status === 401) {
                this.handleAuthError();
                throw new Error('Admin session expired. Please login again.');
            }

            if (response.status === 403) {
                throw new Error('Admin access required for this operation.');
            }

            if (!response.ok) {
                const errorData = await response.json().catch(() => ({
                    message: `HTTP Error: ${response.status} ${response.statusText}`
                }));
                throw new Error(errorData.message || 'Request failed');
            }

            return await response.json();
        } catch (error) {
            console.error('AdminAPI Request failed:', {
                url: url,
                error: error.message,
                options: options
            });
            throw error;
        }
    }

    // Cache management untuk performance optimization
    getCachedData(key) {
        const cached = this.cache.get(key);
        if (cached && (Date.now() - cached.timestamp) < this.cacheTimeout) {
            return cached.data;
        }
        return null;
    }

    setCachedData(key, data) {
        this.cache.set(key, {
            data: data,
            timestamp: Date.now()
        });
    }

    clearCache() {
        this.cache.clear();
    }

    // Get comprehensive user statistics untuk dashboard
    async getAdminUserStats() {
        const cacheKey = 'admin_user_stats';
        const cached = this.getCachedData(cacheKey);
        if (cached) return cached;

        try {
            const result = await this.request(`${this.endpoints.auth}/admin/users/stats`);
            this.setCachedData(cacheKey, result);
            return result;
        } catch (error) {
            console.error('Failed to get admin user stats:', error);
            throw new Error('Failed to load user statistics');
        }
    }

    // Get users list dengan filtering untuk admin management
    async getAdminUsers(page = 1, perPage = 20, search = '', roleFilter = '') {
        const params = new URLSearchParams({
            page: page.toString(),
            per_page: perPage.toString(),
            ...(search && { search }),
            ...(roleFilter && { role: roleFilter })
        });

        try {
            const result = await this.request(`${this.endpoints.auth}/admin/users?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get admin users:', error);
            throw new Error('Failed to load users list');
        }
    }

    // Get user activity feed untuk monitoring
    async getAdminActivityFeed(limit = 20, activityType = 'all') {
        const params = new URLSearchParams({
            limit: limit.toString(),
            ...(activityType !== 'all' && { type: activityType })
        });

        try {
            const result = await this.request(`${this.endpoints.auth}/admin/activity?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get activity feed:', error);
            throw new Error('Failed to load activity feed');
        }
    }

    // Update user status (activate/deactivate)
    async updateUserStatus(userId, isActive) {
        try {
            const result = await this.request(`${this.endpoints.auth}/admin/users/status`, {
                method: 'PUT',
                body: JSON.stringify({
                    user_id: userId,
                    is_active: isActive
                })
            });

            // Clear users cache after update
            this.clearCache();
            return result;
        } catch (error) {
            console.error('Failed to update user status:', error);
            throw new Error('Failed to update user status');
        }
    }

    // Get comprehensive book statistics untuk dashboard
    async getAdminBookStats() {
        const cacheKey = 'admin_book_stats';
        const cached = this.getCachedData(cacheKey);
        if (cached) return cached;

        try {
            const result = await this.request(`${this.endpoints.books}/admin/books/stats`);
            this.setCachedData(cacheKey, result);
            return result;
        } catch (error) {
            console.error('Failed to get admin book stats:', error);
            throw new Error('Failed to load book statistics');
        }
    }

    // Get top books by different metrics
    async getTopBooks(metric = 'downloads', limit = 10) {
        const params = new URLSearchParams({
            metric: metric,
            limit: limit.toString()
        });

        try {
            const result = await this.request(`${this.endpoints.books}/admin/books/top?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get top books:', error);
            throw new Error('Failed to load top books');
        }
    }

    // Get sales analytics untuk chart visualization
    async getSalesAnalytics(days = 30) {
        const params = new URLSearchParams({
            days: days.toString()
        });

        try {
            const result = await this.request(`${this.endpoints.books}/admin/analytics/sales?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get sales analytics:', error);
            throw new Error('Failed to load sales analytics');
        }
    }

    // Get popular books chart data untuk dashboard visualization
    async getPopularBooksChartData(limit = 10) {
        const params = new URLSearchParams({
            limit: limit.toString()
        });

        try {
            const result = await this.request(`${this.endpoints.books}/admin/books/chart?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get popular books chart:', error);
            throw new Error('Failed to load popular books chart');
        }
    }

    // Get category analytics untuk performance insights
    async getCategoryAnalytics() {
        const cacheKey = 'category_analytics';
        const cached = this.getCachedData(cacheKey);
        if (cached) return cached;

        try {
            const result = await this.request(`${this.endpoints.books}/admin/categories/analytics`);
            this.setCachedData(cacheKey, result);
            return result;
        } catch (error) {
            console.error('Failed to get category analytics:', error);
            throw new Error('Failed to load category analytics');
        }
    }

    // Get dashboard metrics (combined book data)
    async getBookDashboardMetrics() {
        try {
            const result = await this.request(`${this.endpoints.books}/admin/dashboard/metrics`);
            return result;
        } catch (error) {
            console.error('Failed to get book dashboard metrics:', error);
            throw new Error('Failed to load book dashboard metrics');
        }
    }

    // Get comprehensive order statistics untuk dashboard
    async getAdminOrderStats() {
        const cacheKey = 'admin_order_stats';
        const cached = this.getCachedData(cacheKey);
        if (cached) return cached;

        try {
            const result = await this.request(`${this.endpoints.payments}/admin/orders/stats`);
            this.setCachedData(cacheKey, result);
            return result;
        } catch (error) {
            console.error('Failed to get admin order stats:', error);
            throw new Error('Failed to load order statistics');
        }
    }

    // Get revenue analytics untuk dashboard charts
    async getRevenueAnalytics(period = 'monthly', days = 30) {
        const params = new URLSearchParams({
            period: period,
            days: days.toString()
        });

        try {
            const result = await this.request(`${this.endpoints.payments}/admin/analytics/revenue?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get revenue analytics:', error);
            throw new Error('Failed to load revenue analytics');
        }
    }

    // Get sales chart data untuk Chart.js visualization
    async getSalesChartData(days = 30) {
        const params = new URLSearchParams({
            days: days.toString()
        });

        try {
            const result = await this.request(`${this.endpoints.payments}/admin/analytics/sales?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get sales chart data:', error);
            throw new Error('Failed to load sales chart data');
        }
    }

    // Get recent orders untuk dashboard display
    async getRecentOrders(limit = 10, statusFilter = '') {
        const params = new URLSearchParams({
            limit: limit.toString(),
            ...(statusFilter && { status: statusFilter })
        });

        try {
            const result = await this.request(`${this.endpoints.payments}/admin/orders/recent?${params}`);
            return result;
        } catch (error) {
            console.error('Failed to get recent orders:', error);
            throw new Error('Failed to load recent orders');
        }
    }

    // Update order status (admin action)
    async updateOrderStatus(orderId, status, notes = null) {
        try {
            const payload = { status };
            if (notes) payload.notes = notes;

            const result = await this.request(`${this.endpoints.payments}/admin/orders/${orderId}/status`, {
                method: 'PUT',
                body: JSON.stringify(payload)
            });

            // Clear order cache after update
            this.clearCache();
            return result;
        } catch (error) {
            console.error('Failed to update order status:', error);
            throw new Error('Failed to update order status');
        }
    }

    // Get system health status
    async getSystemHealth() {
        try {
            const result = await this.request(`${this.endpoints.payments}/admin/system/health`);
            return result;
        } catch (error) {
            console.error('Failed to get system health:', error);
            // Return degraded state instead of throwing
            return {
                success: false,
                data: {
                    database_connected: false,
                    cache_status: { connected: false },
                    service_version: 'unknown',
                    timestamp: new Date().toISOString()
                }
            };
        }
    }

    // Trigger maintenance job manually
    async triggerMaintenance() {
        try {
            const result = await this.request(`${this.endpoints.payments}/admin/maintenance/trigger`, {
                method: 'POST'
            });
            return result;
        } catch (error) {
            console.error('Failed to trigger maintenance:', error);
            throw new Error('Failed to trigger maintenance job');
        }
    }

    // Get scheduler status
    async getSchedulerStatus() {
        try {
            const result = await this.request(`${this.endpoints.payments}/admin/scheduler/status`);
            return result;
        } catch (error) {
            console.error('Failed to get scheduler status:', error);
            throw new Error('Failed to load scheduler status');
        }
    }

    // Get real-time metrics untuk dashboard header
    async getRealTimeMetrics() {
        try {
            const result = await this.request(`${this.endpoints.payments}/admin/metrics/realtime`);
            return result;
        } catch (error) {
            console.error('Failed to get real-time metrics:', error);
            // Return fallback data instead of throwing error untuk graceful degradation
            return {
                success: true,
                data: {
                    online_users: '--',
                    today_sales: 0,
                    today_orders: 0,
                    pending_payments: 0
                }
            };
        }
    }

    // Get payment method analytics
    async getPaymentMethodAnalytics() {
        const cacheKey = 'payment_method_analytics';
        const cached = this.getCachedData(cacheKey);
        if (cached) return cached;

        try {
            const result = await this.request(`${this.endpoints.payments}/admin/analytics/payment-methods`);
            this.setCachedData(cacheKey, result);
            return result;
        } catch (error) {
            console.error('Failed to get payment method analytics:', error);
            throw new Error('Failed to load payment method analytics');
        }
    }

    // Get comprehensive admin statistics (combines all services)
    async getAdminStats() {
        try {
            const [userStats, bookStats, orderStats] = await Promise.all([
                this.getAdminUserStats().catch(err => {
                    console.warn('User stats failed:', err);
                    return null;
                }),
                this.getAdminBookStats().catch(err => {
                    console.warn('Book stats failed:', err);
                    return null;
                }),
                this.getAdminOrderStats().catch(err => {
                    console.warn('Order stats failed:', err);
                    return null;
                })
            ]);

            return {
                success: true,
                data: {
                    users: (userStats && userStats.data) || {
                        total: 0,
                        monthly_change: 0
                    },
                    books: (bookStats && bookStats.data) || {
                        total: 0,
                        monthly_change: 0
                    },
                    orders: (orderStats && orderStats.data) || {
                        total: 0,
                        monthly_change: 0
                    },
                    revenue: (orderStats && orderStats.data) || {
                        total: 0,
                        monthly_change: 0
                    },
                    system: {
                        services_status: {
                            'auth-service': { status: 'online' },
                            'book-service': { status: 'online' },
                            'payment-service': { status: 'online' },
                            'database': { status: 'online' }
                        }
                    }
                }
            };
        } catch (error) {
            console.error('Failed to get combined admin stats:', error);
            throw new Error('Failed to load dashboard statistics');
        }
    }

    // Get revenue chart data (enhanced dari payment service)
    async getRevenueChartData(period = 'monthly') {
        try {
            // Get revenue analytics dengan specified period
            const revenueData = await this.getRevenueAnalytics(period);

            if (!revenueData.success || !revenueData.data) {
                throw new Error('Invalid revenue data received');
            }

            // Transform untuk Chart.js format
            const analytics = revenueData.data;

            return {
                success: true,
                data: {
                    labels: analytics.data_points.map(point => point.date),
                    datasets: [{
                            label: 'Revenue',
                            data: analytics.data_points.map(point => parseFloat(point.revenue)),
                            borderColor: 'rgb(99, 102, 241)',
                            backgroundColor: 'rgba(99, 102, 241, 0.1)',
                            fill: true,
                            tension: 0.4,
                            yAxisID: 'y'
                        },
                        {
                            label: 'Orders',
                            data: analytics.data_points.map(point => point.orders_count),
                            borderColor: 'rgb(16, 185, 129)',
                            backgroundColor: 'rgba(16, 185, 129, 0.1)',
                            fill: false,
                            tension: 0.4,
                            yAxisID: 'y1'
                        }
                    ]
                }
            };
        } catch (error) {
            console.error('Failed to get revenue chart data:', error);
            throw new Error('Failed to load revenue chart data');
        }
    }

    // Setup real-time updates untuk dashboard
    setupRealTimeUpdates(callback) {
        if (typeof callback !== 'function') {
            console.error('Real-time update callback must be a function');
            return null;
        }

        // Add callback ke set
        this.realTimeCallbacks.add(callback);

        // Start real-time updates jika belum aktif
        if (!this.realTimeInterval) {
            this.realTimeInterval = setInterval(async() => {
                try {
                    // Get real-time metrics
                    const metrics = await this.getRealTimeMetrics();

                    // Notify all callbacks
                    this.realTimeCallbacks.forEach(cb => {
                        try {
                            cb('metrics_update', metrics.data);
                        } catch (error) {
                            console.error('Real-time callback error:', error);
                        }
                    });
                } catch (error) {
                    console.warn('Real-time update failed:', error);
                }
            }, 30000); // Update every 30 seconds
        }

        // Return cleanup function
        return () => {
            this.realTimeCallbacks.delete(callback);

            // Stop interval jika tidak ada callbacks
            if (this.realTimeCallbacks.size === 0 && this.realTimeInterval) {
                clearInterval(this.realTimeInterval);
                this.realTimeInterval = null;
            }
        };
    }

    // Validate admin permissions untuk specific operations
    async validateAdminPermissions(operation) {
        try {
            // Simple permission check - could be enhanced
            if (!this.token) {
                throw new Error('No authentication token available');
            }

            return {
                success: true,
                has_permission: true,
                operation: operation
            };
        } catch (error) {
            console.error('Permission validation failed:', error);
            throw new Error('Permission validation failed');
        }
    }

    // Export admin data untuk backup/analytics
    async exportAdminData(dataType = 'all', format = 'json') {
        try {
            // Validate admin permissions
            await this.validateAdminPermissions('export_data');

            let exportData = {};

            // Collect data based on type
            if (dataType === 'all' || dataType === 'users') {
                exportData.users = await this.getAdminUserStats();
            }

            if (dataType === 'all' || dataType === 'books') {
                exportData.books = await this.getAdminBookStats();
            }

            if (dataType === 'all' || dataType === 'orders') {
                exportData.orders = await this.getAdminOrderStats();
            }

            if (dataType === 'all' || dataType === 'analytics') {
                exportData.analytics = {
                    revenue: await this.getRevenueAnalytics(),
                    categories: await this.getCategoryAnalytics(),
                    payment_methods: await this.getPaymentMethodAnalytics()
                };
            }

            // Add export metadata
            exportData.export_info = {
                timestamp: new Date().toISOString(),
                data_type: dataType,
                format: format,
                version: '1.0.0'
            };

            // Format based on requested format
            if (format === 'csv') {
                return this.convertToCSV(exportData);
            }

            return JSON.stringify(exportData, null, 2);
        } catch (error) {
            console.error('Export failed:', error);
            throw new Error('Failed to export admin data');
        }
    }

    // Get cache statistics untuk performance monitoring
    getCacheStats() {
        return {
            total_entries: this.cache.size,
            cache_timeout: this.cacheTimeout,
            memory_usage: JSON.stringify([...this.cache.entries()]).length,
            oldest_entry: this.cache.size > 0 ?
                Math.min(...[...this.cache.values()].map(v => v.timestamp)) : null
        };
    }

    // Handle authentication errors
    handleAuthError() {
        // Clear stored token
        this.token = null;
        localStorage.removeItem('auth_token');

        // Redirect to login (could be handled by calling code)
        console.warn('Admin authentication expired');
    }

    // Convert data to CSV format
    convertToCSV(data) {
        // Simplified CSV conversion - could be enhanced
        try {
            const flatData = this.flattenObject(data);
            const headers = Object.keys(flatData);
            const values = Object.values(flatData);

            return [
                headers.join(','),
                values.join(',')
            ].join('\n');
        } catch (error) {
            console.error('CSV conversion failed:', error);
            return JSON.stringify(data); // Fallback to JSON
        }
    }

    // Flatten nested object untuk CSV conversion
    flattenObject(obj, prefix = '') {
        let result = {};

        for (const key in obj) {
            if (obj.hasOwnProperty(key)) {
                const newKey = prefix ? `${prefix}.${key}` : key;

                if (typeof obj[key] === 'object' && obj[key] !== null && !Array.isArray(obj[key])) {
                    Object.assign(result, this.flattenObject(obj[key], newKey));
                } else {
                    result[newKey] = obj[key];
                }
            }
        }

        return result;
    }

    // Cleanup resources
    cleanup() {
        // Clear real-time updates
        if (this.realTimeInterval) {
            clearInterval(this.realTimeInterval);
            this.realTimeInterval = null;
        }

        // Clear callbacks
        this.realTimeCallbacks.clear();

        // Clear cache
        this.clearCache();

        console.log('AdminAPI cleanup completed');
    }
}

// Export AdminAPI class untuk global access
window.AdminAPI = AdminAPI;