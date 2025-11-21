// /pdf-bookstore/frontend/js/cart.js

// Shopping cart manager class untuk handle cart operations dan purchase flow
class CartManager {
    constructor(api, auth) {
        this.api = api;
        this.auth = auth;
        this.cart = new Map();
        this.cartCallbacks = [];
        this.storageKey = 'bookstore_cart';

        // Load existing cart dari localStorage
        this.loadCartFromStorage();

        // Setup auth Listener untuk cart cleanup on logout
        this.auth.onLogout(() => this.clearCart()); // Clear cart ketika user logout
    }

    // Function untuk add book ke cart dengan validation 
    async addToCart(bookId, quantity = 1) {
        try {
            // get book details dari book service untuk validation 
            const bookResponse = await this.api.getBook(bookId);

            if (!bookResponse.success || !bookResponse.data) {
                throw new Error('Book not found');
            }

            let book;
            if (bookResponse.data.book) {
                book = bookResponse.data.book;
            } else if (bookResponse.data.title) {
                book = bookResponse.data;
            } else {
                throw new Error('Invalid book response structure');
            }

            // Check if book is active dan available
            if (!book.is_active) {
                throw new Error('This book is not available for purchase');
            }

            // For digital books, quantity should always be 1
            const validQuantity = Math.max(1, Math.min(quantity, 1));

            // Check if book already in cart 
            if (this.cart.has(bookId)) {
                // update existing cart item quantity
                const existingItem = this.cart.get(bookId);
                existingItem.quantity = validQuantity;
                existingItem.updateAt = new Date().toISOString();
            } else {
                // add new item ke cart 
                const cartItem = {
                    bookId: bookId,
                    title: book.title,
                    author: book.author,
                    price: parseFloat(book.price),
                    coverPath: book.cover_path,
                    quantity: validQuantity,
                    addedAt: new Date().toISOString(),
                    updateAt: new Date().toISOString(),
                };

                this.cart.set(bookId, cartItem);
            }

            // save cart ke localstorage untuk persistence
            this.saveCartToStorage();

            // Trigger cart change callbacks untuk UI updates
            this.triggerCartCallbacks();

            return {
                success: true,
                message: 'Book added to cart succesfully',
                cartSize: this.cart.size
            };
        } catch (error) {
            console.error('Add to cart failed:', error);
            throw new Error(error.message || 'Failed to add book to cart');
        }
    }

    // Fucntion untuk remove book dari cart 
    removeFromCart(bookId) {
        // check if book exists dalam cart 
        if (!this.cart.has(bookId)) {
            throw new Error('Book not found in cart');
        }

        // Remove book dari cart Map 
        this.cart.delete(bookId);

        // Save updated cart ke localStorage
        this.saveCartToStorage();

        // trigger cart change callbacks
        this.triggerCartCallbacks();

        return {
            success: true,
            message: 'Book removed from cart',
            cartSize: this.cart.size
        };
    }

    // Function untuk update quantity book dalam cart
    updateQuantity(bookId, quantity) {
        // Validate quantity untuk digital books (always 1)
        const validQuantity = Math.max(1, Math.min(quantity, 1));


        // Check if book exists dalam cart
        if (!this.cart.has(bookId)) {
            throw new Error('Book not found in cart');
        }

        // Update quantity dalam cart item
        const cartItem = this.cart.get(bookId);
        cartItem.quantity = validQuantity;
        cartItem.updatedAt = new Date().toISOString(); // update timestamp

        // Save updated cart ke localStorage
        this.saveCartToStorage(); // persist quantity change

        // Trigger cart change callbacks
        this.triggerCartCallbacks(); // notify UI tentang update

        return {
            success: true,
            message: 'Cart updated successfully',
            item: cartItem // return updated cart item
        };
    }

    // Function untuk get all cart items sebagai array
    getCartItems() {
        // Convert Map ke Array untuk easier iteration
        return Array.from(this.cart.values()); // return cart items sebagai array
    }

    // Function untuk get cart item by book ID
    getCartItem(bookId) {
        return this.cart.get(bookId) || null; // return cart item atau null jika not found
    }

    // Function untuk check if book is dalam cart
    isInCart(bookId) {
        return this.cart.has(bookId); // return boolean untuk cart membership check
    }

    // Function untuk get cart size
    getCartSize() {
        return this.cart.size; // return number of items dalam cart
    }

    // Function untuk calculate total cart price
    calculateTotal() {
        let total = 0; // initialize total amount

        // Sum all cart item prices dengan quantity
        this.cart.forEach(item => {
            total += item.price * item.quantity; // add item total ke overall total
        });

        return parseFloat(total.toFixed(2)); // return total dengan 2 decimal places
    }

    // Function untuk clear entire cart
    clearCart() {
        this.cart.clear(); // remove all items dari cart Map

        // Clear cart dari localStorage
        localStorage.removeItem(this.storageKey); // remove persisted cart data

        // Trigger cart change callbacks
        this.triggerCartCallbacks(); // notify UI tentang cart clear

        return {
            success: true,
            message: 'Cart cleared successfully'
        };
    }

    // Function untuk create order dari cart items dengan payment processing
    async processCheckout(paymentMethod = 'qris') {
        // Check authentication before checkout
        if (!this.auth.isAuthenticated()) {
            throw new Error('Please log in to continue with checkout');
        }

        // Check if cart is not empty
        if (this.cart.size === 0) {
            throw new Error('Your cart is empty. Please add books before checkout.');
        }

        // For now, process first item dalam cart (single book purchase)

        const cartItems = this.getCartItems();
        const firstItem = cartItems[0];

        if (!firstItem) {
            throw new Error('No items found in cart');
        }

        try {
            // Create order dengan payment service
            const orderResponse = await this.api.createOrder(firstItem.bookId, paymentMethod);
            // ^^^^^^^^^^^^^ = response dari payment-service/handlers.rs line 25

            if (!orderResponse.success) {
                throw new Error(orderResponse.message || 'Failed to create order');
            }

            const order = orderResponse.data; // extract order data dari response

            // Remove purchased book dari cart setelah successful order
            this.removeFromCart(firstItem.bookId); // clean up cart after purchase

            return {
                success: true,
                order: order, // return order data untuk payment redirect
                paymentUrl: order.order.payment_url, // extract payment URL untuk redirect
                message: 'Order created successfully. Redirecting to payment...'
            };
        } catch (error) {
            console.error('Checkout failed:', error);
            throw new Error(error.message || 'Checkout failed. Please try again.');
        }
    }

    // Function untuk save cart ke localStorage untuk persistence
    saveCartToStorage() {
        try {
            // Convert Map ke Object untuk JSON serialization
            const cartObject = Object.fromEntries(this.cart);


            // Save ke localStorage dengan JSON encoding
            localStorage.setItem(this.storageKey, JSON.stringify(cartObject));

        } catch (error) {
            console.warn('Failed to save cart to localStorage:', error);

        }
    }

    // Function untuk load cart dari localStorage
    loadCartFromStorage() {
        try {
            // Get cart data dari localStorage
            const cartData = localStorage.getItem(this.storageKey);


            if (cartData) {
                // Parse JSON dan convert ke Map
                const cartObject = JSON.parse(cartData);


                // Reconstruct Map dari stored object
                this.cart = new Map(Object.entries(cartObject));


                // Validate cart items dan remove invalid ones
                this.validateCartItems(); // clean up invalid items
            }
        } catch (error) {
            console.warn('Failed to load cart from localStorage:', error);
            this.cart = new Map();
        }
    }

    // Function untuk validate cart items dan remove invalid books
    async validateCartItems() {
        const invalidItems = [];

        // Check each cart item untuk validity
        for (const [bookId, item] of this.cart) {
            try {
                // Verify book masih exists dan active
                const bookResponse = await this.api.getBook(bookId);


                if (!bookResponse.success || !bookResponse.data || !bookResponse.data.book.is_active) {
                    invalidItems.push(bookId);
                }
            } catch (error) {
                // Book tidak ditemukan atau error, mark untuk removal
                invalidItems.push(bookId);
            }
        }

        // Remove invalid items dari cart
        invalidItems.forEach(bookId => {
            this.cart.delete(bookId);
        });

        // Save cleaned cart jika ada removals
        if (invalidItems.length > 0) {
            this.saveCartToStorage();
            this.triggerCartCallbacks();
        }
    }

    // Function untuk register callback untuk cart change events
    onCartChange(callback) {
        this.cartCallbacks.push(callback);

        // Trigger callback immediately dengan current cart state
        callback(this.getCartItems(), this.calculateTotal());

    }

    // Function untuk remove callback dari cart events
    removeCartCallback(callback) {
        this.cartCallbacks = this.cartCallbacks.filter(cb => cb !== callback);

    }

    // Function untuk trigger all registered cart callbacks
    triggerCartCallbacks() {
        const cartItems = this.getCartItems();
        const total = this.calculateTotal();

        this.cartCallbacks.forEach(callback => {
            try {
                callback(cartItems, total);
            } catch (error) {
                console.error('Cart callback error:', error);

            }
        });
    }

    // Function untuk get cart summary untuk display
    getCartSummary() {
        const items = this.getCartItems();
        const total = this.calculateTotal();
        const itemCount = this.getCartSize();

        return {
            items: items,
            total: total,
            itemCount: itemCount,
            isEmpty: itemCount === 0
        };
    }

    // Function untuk check if user has purchased specific book
    async checkBookOwnership(bookId) {
        // Check authentication first
        if (!this.auth.isAuthenticated()) {
            return false;
        }

        try {

            const response = await this.api.checkPurchaseStatus(bookId);


            return response.success && response.has_purchased;

        } catch (error) {
            console.warn('Failed to check book ownership:', error);
            return false;
        }
    }

    // Function untuk get cart storage usage information
    getStorageInfo() {
        try {
            const cartData = localStorage.getItem(this.storageKey);

            const sizeInBytes = new Blob([cartData || '']).size;


            return {
                hasData: !!cartData,
                sizeInBytes: sizeInBytes,
                itemCount: this.cart.size
            };
        } catch (error) {
            return {
                hasData: false,
                sizeInBytes: 0,
                itemCount: 0,
                error: error.message
            };
        }
    }
}