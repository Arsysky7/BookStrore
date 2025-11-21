// /pdf-bookstore/frontend/js/payment.js

// Payment manager class untuk handle payment flow dan Midtrans integration
class PaymentManager {
    constructor(api, auth) {
        this.api = api;
        this.auth = auth;
        this.paymentCallbacks = [];
        this.currentOrder = null;
        this.paymentWindow = null;
        this.statusCheckInterval = null;

        // Payment method configurations sesuai dengan backend Midtrans integration
        this.paymentMethods = this.initializePaymentMethods();
    }

    // Function untuk initialize payment methods sesuai dengan midtrans.rs
    initializePaymentMethods() {
        return {
            'qris': {
                id: 'qris',
                name: 'QRIS (Scan to Pay)',
                description: 'Scan QR code dengan aplikasi mobile banking atau e-wallet',
                icon: 'qris-icon',
                category: 'instant',
                estimatedTime: '1-2 minutes',
                priority: 1
            },
            'gopay': {
                id: 'gopay',
                name: 'GoPay',
                description: 'Bayar dengan saldo GoPay atau GoPay Later',
                icon: 'gopay-icon',
                category: 'e_wallet',
                estimatedTime: '1-3 minutes',
                priority: 2
            },
            'shopeepay': {
                id: 'shopeepay',
                name: 'ShopeePay',
                description: 'Bayar dengan saldo ShopeePay',
                icon: 'shopeepay-icon',
                category: 'e_wallet',
                estimatedTime: '1-3 minutes',
                priority: 3
            },
            'bank_transfer': {
                id: 'bank_transfer',
                name: 'Virtual Account',
                description: 'Transfer melalui ATM, mobile banking, atau internet banking',
                icon: 'bank-icon',
                category: 'bank_transfer',
                estimatedTime: '5-15 minutes',
                priority: 4
            },
            'credit_card': {
                id: 'credit_card',
                name: 'Credit/Debit Card',
                description: 'Visa, Mastercard, JCB cards',
                icon: 'card-icon',
                category: 'card',
                estimatedTime: '1-2 minutes',
                priority: 5
            },
            'convenience_store': {
                id: 'convenience_store',
                name: 'Convenience Store',
                description: 'Bayar di Indomaret, Alfamart terdekat',
                icon: 'store-icon',
                category: 'offline',
                estimatedTime: '1-24 hours',
                priority: 6
            }
        };
    }

    // Function untuk get available payment methods untuk selection UI
    getPaymentMethods() {
        return Object.values(this.paymentMethods)
            .sort((a, b) => a.priority - b.priority);
    }

    // Function untuk get payment method by ID
    getPaymentMethod(methodId) {
        return this.paymentMethods[methodId] || null;
    }

    // Function untuk initiate payment process sesuai dengan create_order handler
    async initiatePayment(bookId, paymentMethodId) {
        // Validate authentication
        if (!this.auth.isAuthenticated()) {
            throw new Error('Please log in to continue with payment');
        }

        // Validate payment method
        const paymentMethod = this.getPaymentMethod(paymentMethodId);
        if (!paymentMethod) {
            throw new Error('Invalid payment method selected');
        }

        try {
            // Create order dengan payment service sesuai CreateOrderRequest
            const orderResponse = await this.api.createOrder(bookId, paymentMethodId);

            if (!orderResponse.success || !orderResponse.data) {
                throw new Error(orderResponse.message || 'Failed to create order');
            }

            this.currentOrder = orderResponse.data;

            // Trigger payment callbacks untuk UI updates
            this.triggerPaymentCallbacks('order_created', this.currentOrder);

            return {
                success: true,
                order: this.currentOrder,
                paymentUrl: this.currentOrder.order.payment_url, // sesuai Order struct
                message: 'Order created successfully. Redirecting to payment...'
            };
        } catch (error) {
            console.error('Payment initiation failed:', error);
            throw new Error(error.message || 'Failed to initiate payment. Please try again.');
        }
    }

    // Function untuk open payment window dan handle payment flow
    async openPaymentWindow(paymentUrl) {
        // Validate payment URL dari Midtrans
        if (!paymentUrl) {
            throw new Error('Payment URL not available');
        }

        try {
            // Calculate popup window dimensions dan position
            const width = 800;
            const height = 600;
            const left = (screen.width - width) / 2;
            const top = (screen.height - height) / 2;

            // Open payment popup window untuk Midtrans payment page
            this.paymentWindow = window.open(
                paymentUrl,
                'midtrans_payment',
                `width=${width},height=${height},left=${left},top=${top},resizable=yes,scrollbars=yes,status=yes`
            );

            // Check if popup was blocked oleh browser
            if (!this.paymentWindow) {
                throw new Error('Payment window was blocked. Please allow popups and try again.');
            }

            // Start monitoring payment window untuk completion detection
            this.startPaymentMonitoring();

            // Trigger payment callbacks untuk UI state
            this.triggerPaymentCallbacks('payment_window_opened', this.currentOrder);

            return {
                success: true,
                message: 'Payment window opened. Please complete your payment.'
            };
        } catch (error) {
            console.error('Failed to open payment window:', error);
            throw new Error(error.message || 'Failed to open payment window');
        }
    }

    // Function untuk start payment monitoring dengan polling dan window tracking
    startPaymentMonitoring() {
        // Start polling payment status dari backend
        this.startStatusPolling();

        // Monitor payment window untuk closure detection
        const windowCheckInterval = setInterval(() => {
            if (this.paymentWindow && this.paymentWindow.closed) {
                // Payment window closed, check final status
                clearInterval(windowCheckInterval);

                // Trigger callback untuk window closure
                this.triggerPaymentCallbacks('payment_window_closed', this.currentOrder);

                // Final status check setelah window close
                setTimeout(() => {
                    this.checkPaymentStatus();
                }, 2000);
            }
        }, 1000);
    }

    // Function untuk start status polling sesuai dengan order status endpoint
    startStatusPolling() {
        // Clear existing polling interval jika ada
        if (this.statusCheckInterval) {
            clearInterval(this.statusCheckInterval);
        }

        // Start polling payment status setiap 5 seconds
        this.statusCheckInterval = setInterval(async() => {
            try {
                await this.checkPaymentStatus();
            } catch (error) {
                console.warn('Status check failed:', error);
            }
        }, 5000);

        // Auto-stop polling setelah 30 minutes untuk prevent infinite polling
        setTimeout(() => {
            this.stopStatusPolling();
        }, 30 * 60 * 1000);
    }

    // Function untuk stop status polling
    stopStatusPolling() {
        if (this.statusCheckInterval) {
            clearInterval(this.statusCheckInterval);
            this.statusCheckInterval = null;
        }
    }

    // Function untuk check payment status dengan payment service
    async checkPaymentStatus() {
        // Validate current order existence
        if (!this.currentOrder) {
            return null;
        }

        try {
            // Get order status dari payment service sesuai get_order_status handler
            const statusResponse = await this.api.getOrder(this.currentOrder.order.id);

            if (statusResponse.success && statusResponse.data) {
                const updatedOrder = statusResponse.data;
                const orderStatus = updatedOrder.order.status;

                // Check jika status berubah dari current order
                if (orderStatus !== this.currentOrder.order.status) {
                    this.currentOrder = updatedOrder;

                    // Handle different payment statuses sesuai PaymentStatus enum
                    switch (orderStatus) {
                        case 'paid':
                            this.handlePaymentSuccess(updatedOrder);
                            break;
                        case 'failed':
                        case 'cancelled':
                        case 'expired':
                            this.handlePaymentFailure(updatedOrder, orderStatus);
                            break;
                        case 'pending':
                            this.handlePaymentPending(updatedOrder);
                            break;
                        default:
                            console.warn('Unknown payment status:', orderStatus);
                    }

                    // Trigger status change callback
                    this.triggerPaymentCallbacks('status_changed', updatedOrder);
                }

                return updatedOrder;
            }
        } catch (error) {
            console.error('Payment status check failed:', error);
            return null;
        }
    }

    // Function untuk handle successful payment
    handlePaymentSuccess(order) {
        // Stop status polling karena payment complete
        this.stopStatusPolling();

        // Close payment window jika masih open
        if (this.paymentWindow && !this.paymentWindow.closed) {
            this.paymentWindow.close();
        }

        // Trigger success callback dengan order data
        this.triggerPaymentCallbacks('payment_success', order);

        // Show success message dengan download option
        const bookTitle = order.book_title || 'your book';
        if (confirm(`Payment successful! ${bookTitle} is now available in your library. Go to library now?`)) {
            window.location.href = '/library.html';
        }

        // Reset current order
        this.currentOrder = null;
    }

    // Function untuk handle failed payment
    handlePaymentFailure(order, status) {
        // Stop status polling karena payment final
        this.stopStatusPolling();

        // Close payment window jika masih open
        if (this.paymentWindow && !this.paymentWindow.closed) {
            this.paymentWindow.close();
        }

        // Trigger failure callback dengan order data dan status
        this.triggerPaymentCallbacks('payment_failed', order, status);

        // Show appropriate error message based on status
        let message = 'Payment failed. Please try again.';
        switch (status) {
            case 'cancelled':
                message = 'Payment was cancelled.';
                break;
            case 'expired':
                message = 'Payment session expired. Please try again.';
                break;
            case 'failed':
                message = 'Payment failed. Please check your payment details and try again.';
                break;
        }

        // Reset current order
        this.currentOrder = null;
    }

    // Function untuk handle pending payment
    handlePaymentPending(order) {
        // Trigger pending callback untuk UI updates
        this.triggerPaymentCallbacks('payment_pending', order);

        // Show pending message dengan payment details
        const paymentMethod = this.getPaymentMethod(order.order.payment_method);
        const estimatedTime = paymentMethod ? paymentMethod.estimatedTime : '5-15 minutes';

        console.log(`Payment is being processed. Estimated completion time: ${estimatedTime}`);
    }

    // Function untuk cancel current payment process
    async cancelPayment() {
        // Validate current order existence
        if (!this.currentOrder) {
            throw new Error('No active payment to cancel');
        }

        try {
            // Send cancel request ke payment service sesuai cancel_order handler
            const cancelResponse = await this.api.cancelOrder(this.currentOrder.order.id);

            if (cancelResponse.success) {
                // Stop status polling
                this.stopStatusPolling();

                // Close payment window jika open
                if (this.paymentWindow && !this.paymentWindow.closed) {
                    this.paymentWindow.close();
                }

                // Trigger cancellation callback
                this.triggerPaymentCallbacks('payment_cancelled', this.currentOrder);

                // Reset current order
                this.currentOrder = null;

                return {
                    success: true,
                    message: 'Payment cancelled successfully'
                };
            } else {
                throw new Error(cancelResponse.message || 'Failed to cancel payment');
            }
        } catch (error) {
            console.error('Payment cancellation failed:', error);
            throw new Error(error.message || 'Failed to cancel payment');
        }
    }

    // Function untuk get current payment status
    getCurrentPaymentStatus() {
        if (!this.currentOrder) {
            return null;
        }

        return {
            order: this.currentOrder,
            status: this.currentOrder.order.status,
            isPolling: !!this.statusCheckInterval,
            hasPaymentWindow: this.paymentWindow && !this.paymentWindow.closed,
            orderNumber: this.currentOrder.order.order_number,
            amount: this.currentOrder.order.amount,
            paymentMethod: this.currentOrder.order.payment_method
        };
    }

    // Function untuk register callback untuk payment events
    onPaymentEvent(callback) {
        this.paymentCallbacks.push(callback);
    }

    // Function untuk remove callback dari payment events
    removePaymentCallback(callback) {
        this.paymentCallbacks = this.paymentCallbacks.filter(cb => cb !== callback);
    }

    // Function untuk trigger all registered payment callbacks
    triggerPaymentCallbacks(eventType, order, additionalData = null) {
        this.paymentCallbacks.forEach(callback => {
            try {
                // Call callback dengan event type, order data, dan additional data
                callback(eventType, order, additionalData);
            } catch (error) {
                console.error('Payment callback error:', error);
            }
        });
    }

    // Function untuk format payment amount untuk display sesuai Indonesian format
    formatAmount(amount) {
        // Convert BigDecimal amount ke Indonesian Rupiah format
        let numericAmount;

        if (typeof amount === 'string') {
            numericAmount = parseFloat(amount);
            if (isNaN(numericAmount)) {
                console.warn('Invalid amount format:', amount);
                numericAmount = 0;
            }
        } else if (typeof amount === 'number') {
            numericAmount = amount;
        } else {
            console.warn('Unexpected amount type:', typeof amount, amount);
            numericAmount = 0;
        }

        return new Intl.NumberFormat('id-ID', {
            style: 'currency',
            currency: 'IDR',
            minimumFractionDigits: 0,
            maximumFractionDigits: 0
        }).format(numericAmount);
    }

    // Function untuk calculate payment fee berdasarkan method sesuai Midtrans fees
    calculatePaymentFee(amount, paymentMethodId) {
        let numericAmount;

        if (typeof amount === 'string') {
            numericAmount = parseFloat(amount);
            if (isNaN(numericAmount)) {
                console.warn('Invalid amount for fee calculation:', amount);
                return 0;
            }
        } else if (typeof amount === 'number') {
            numericAmount = amount;
        } else {
            console.warn('Unexpected amount type for fee calculation:', typeof amount);
            return 0;
        }

        // Fee structure untuk different payment methods sesuai Midtrans pricing
        const feeStructure = {
            'qris': 0,
            'gopay': 0,
            'shopeepay': 0,
            'bank_transfer': 4000,
            'credit_card': Math.max(2500, numericAmount * 0.029),
            'convenience_store': 2500
        };

        return feeStructure[paymentMethodId] || 0;
    }

    // Function untuk get payment method recommendations berdasarkan amount
    getRecommendedPaymentMethods(amount) {
        const numericAmount = typeof amount === 'string' ? parseFloat(amount) : amount;
        const recommendations = [];

        // Recommend based pada amount dan efficiency sesuai Indonesian preferences
        if (numericAmount <= 50000) {
            // Small amounts - prefer instant methods
            recommendations.push('qris', 'gopay', 'shopeepay');
        } else if (numericAmount <= 200000) {
            // Medium amounts - balanced options
            recommendations.push('qris', 'bank_transfer', 'gopay', 'credit_card');
        } else {
            // Large amounts - secure methods
            recommendations.push('bank_transfer', 'credit_card', 'qris');
        }

        // Return recommended methods dengan details, filtered untuk valid methods
        return recommendations
            .map(methodId => this.getPaymentMethod(methodId))
            .filter(method => method);
    }

    // Function untuk get total amount including fees
    calculateTotalAmount(baseAmount, paymentMethodId) {
        const numericAmount = typeof baseAmount === 'string' ? parseFloat(baseAmount) : baseAmount;
        const fee = this.calculatePaymentFee(numericAmount, paymentMethodId);

        return {
            baseAmount: numericAmount,
            fee: fee,
            totalAmount: numericAmount + fee,
            formattedBase: this.formatAmount(numericAmount),
            formattedFee: this.formatAmount(fee),
            formattedTotal: this.formatAmount(numericAmount + fee)
        };
    }

    // Function untuk validate payment amount sesuai Midtrans limits
    validatePaymentAmount(amount) {
        const numericAmount = typeof amount === 'string' ? parseFloat(amount) : amount;

        // Midtrans amount limits
        const minAmount = 1000; // Minimum Rp 1.000
        const maxAmount = 999999999; // Maximum Rp 999.999.999

        if (numericAmount < minAmount) {
            return {
                isValid: false,
                error: `Minimum payment amount is ${this.formatAmount(minAmount)}`
            };
        }

        if (numericAmount > maxAmount) {
            return {
                isValid: false,
                error: `Maximum payment amount is ${this.formatAmount(maxAmount)}`
            };
        }

        return {
            isValid: true,
            error: null
        };
    }

    // Function untuk handle Midtrans webhook simulation (for testing)
    simulateWebhookResponse(orderNumber, status) {
        if (!this.currentOrder || this.currentOrder.order.order_number !== orderNumber) {
            console.warn('Webhook simulation for unknown order:', orderNumber);
            return;
        }

        // Simulate webhook dengan different statuses
        const webhookData = {
            order_id: orderNumber,
            transaction_status: status,
            payment_type: this.currentOrder.order.payment_method,
            gross_amount: this.currentOrder.order.amount.toString()
        };

        console.log('Simulating webhook:', webhookData);

        // Trigger immediate status check
        setTimeout(() => {
            this.checkPaymentStatus();
        }, 1000);
    }

    // Function untuk get payment history untuk user
    async getPaymentHistory(page = 1, limit = 10) {
        try {
            const response = await this.api.getMyOrders(page, limit);

            if (response.success && response.data) {
                return {
                    success: true,
                    orders: response.data,
                    pagination: response.pagination
                };
            } else {
                throw new Error(response.message || 'Failed to get payment history');
            }
        } catch (error) {
            console.error('Failed to get payment history:', error);
            throw new Error(error.message || 'Failed to get payment history');
        }
    }

    // Function untuk retry failed payment dengan same order
    async retryPayment(orderId, newPaymentMethod = null) {
        try {
            // Get order details
            const orderResponse = await this.api.getOrder(orderId);

            if (!orderResponse.success || !orderResponse.data) {
                throw new Error('Order not found');
            }

            const order = orderResponse.data;

            // Check if order can be retried
            if (order.order.status !== 'failed' && order.order.status !== 'cancelled') {
                throw new Error('This order cannot be retried');
            }

            // Use new payment method atau original method
            const paymentMethod = newPaymentMethod || order.order.payment_method;
            const bookId = order.order.book_id;

            // Create new order dengan same book
            return await this.initiatePayment(bookId, paymentMethod);

        } catch (error) {
            console.error('Payment retry failed:', error);
            throw new Error(error.message || 'Failed to retry payment');
        }
    }

    // Function untuk get payment receipt/invoice
    async getPaymentReceipt(orderId) {
        try {
            const orderResponse = await this.api.getOrder(orderId);

            if (!orderResponse.success || !orderResponse.data) {
                throw new Error('Order not found');
            }

            const order = orderResponse.data;

            if (order.order.status !== 'paid') {
                throw new Error('Payment receipt only available for paid orders');
            }

            // Generate receipt data
            return {
                success: true,
                receipt: {
                    orderNumber: order.order.order_number,
                    bookTitle: order.book_title,
                    bookAuthor: order.book_author,
                    amount: order.order.amount,
                    paymentMethod: order.order.payment_method,
                    paidAt: order.order.paid_at,
                    formattedAmount: this.formatAmount(order.order.amount)
                }
            };

        } catch (error) {
            console.error('Failed to get payment receipt:', error);
            throw new Error(error.message || 'Failed to get payment receipt');
        }
    }

    // Function untuk cleanup payment resources
    cleanup() {
        // Stop status polling
        this.stopStatusPolling();

        // Close payment window jika open
        if (this.paymentWindow && !this.paymentWindow.closed) {
            this.paymentWindow.close();
        }

        // Clear payment state
        this.currentOrder = null;
        this.paymentWindow = null;
        this.paymentCallbacks = [];

        console.log('Payment manager cleaned up');
    }

    // Function untuk handle browser visibility change (pause/resume polling)
    handleVisibilityChange() {
        if (document.hidden) {
            // Pause polling when tab is hidden untuk save resources
            if (this.statusCheckInterval) {
                console.log('Pausing payment status polling (tab hidden)');
                this.stopStatusPolling();
            }
        } else {
            // Resume polling when tab becomes visible
            if (this.currentOrder && this.currentOrder.order.status === 'pending') {
                console.log('Resuming payment status polling (tab visible)');
                this.startStatusPolling();
            }
        }
    }

    // Function untuk setup visibility change listener
    setupVisibilityListener() {
        document.addEventListener('visibilitychange', this.handleVisibilityChange.bind(this));
    }

    // Function untuk remove visibility change listener
    removeVisibilityListener() {
        document.removeEventListener('visibilitychange', this.handleVisibilityChange.bind(this));
    }

    // Function untuk get payment method icon URL
    getPaymentMethodIconUrl(methodId) {
        const iconBaseUrl = '/assets/payment-icons/';
        const iconMap = {
            'qris': 'qris.png',
            'gopay': 'gopay.png',
            'shopeepay': 'shopeepay.png',
            'bank_transfer': 'bank-transfer.png',
            'credit_card': 'credit-card.png',
            'convenience_store': 'convenience-store.png'
        };

        return iconBaseUrl + (iconMap[methodId] || 'default.png');
    }

    // Function untuk format payment method display name
    getPaymentMethodDisplayName(methodId) {
        const method = this.getPaymentMethod(methodId);
        return method ? method.name : 'Unknown Payment Method';
    }

    // Function untuk check if payment method is available untuk current amount
    isPaymentMethodAvailable(methodId, amount) {
        const numericAmount = typeof amount === 'string' ? parseFloat(amount) : amount;
        const method = this.getPaymentMethod(methodId);

        if (!method) return false;

        // Check amount limits untuk specific payment methods
        switch (methodId) {
            case 'convenience_store':
                // Convenience store usually has limits
                return numericAmount >= 10000 && numericAmount <= 2500000;
            case 'credit_card':
                // Credit card may have higher minimum
                return numericAmount >= 5000;
            default:
                // Most e-wallets dan VA have standard limits
                return numericAmount >= 1000 && numericAmount <= 999999999;
        }
    }

    // Function untuk get filtered payment methods berdasarkan amount
    getAvailablePaymentMethods(amount) {
        return this.getPaymentMethods().filter(method =>
            this.isPaymentMethodAvailable(method.id, amount)
        );
    }
}

// Setup visibility listener when payment manager is created
PaymentManager.prototype.init = function() {
    this.setupVisibilityListener();
    return this;
};

// Export PaymentManager class untuk global access
window.PaymentManager = PaymentManager;