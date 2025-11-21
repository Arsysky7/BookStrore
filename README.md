**# PDF Digital Bookstore **

A modern microservices-based e-commerce platform for digital books, built with Rust and vanilla JavaScript. Features secure authentication, payment processing, and a comprehensive admin dashboard.

**## =� Features**

### Customer Features
- **Secure Authentication**: JWT-based auth with refresh token rotation, 2FA via OTP
- **Book Discovery**: Browse, search, and filter digital books with pagination
- **Shopping Cart**: Add/remove books, quantity management
- **Payment Processing**: Integrated with Midtrans payment gateway
- **Digital Library**: Access purchased books with built-in PDF reader
- **Reviews & Ratings**: Rate and review purchased books
- **Order History**: Track purchase history and order status

### Admin Features
- **Dashboard**: Real-time analytics and metrics
- **Book Management**: CRUD operations for books and categories
- **File Upload**: Secure PDF and cover image uploads
- **Order Management**: View and manage customer orders
- **User Management**: Admin panel for user administration
- **Sales Reports**: Detailed analytics and reporting

### Technical Features
- **Microservices Architecture**: Scalable service-oriented design
- **API Gateway**: Centralized routing and JWT verification
- **Circuit Breaker**: Fault tolerance for inter-service communication
- **Health Monitoring**: Comprehensive health checks for all services
- **Background Jobs**: Scheduled tasks and async processing
- **Docker Support**: Full containerization with multi-stage builds
