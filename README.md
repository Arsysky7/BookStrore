# PDF Digital Bookstore 

A modern microservices-based e-commerce platform for digital books, built with Rust and vanilla JavaScript. Features secure authentication, payment processing, and a comprehensive admin dashboard.

## About This Project

This is my showcase project that demonstrates building a complete e-commerce system from scratch using modern technologies. It features scalable microservices architecture, secure authentication, payment gateway integration, and smooth user experience.

## What's Built

### Customer Features
- **Login & Registration**: User authentication with JWT and refresh token rotation
- **Browse Books**: Search and explore book collection with pagination
- **Shopping Cart**: Add/remove books with quantity management
- **Payment Processing**: Midtrans integration for secure payments
- **Digital Library**: Access purchased books with built-in PDF reader
- **Reviews & Ratings**: Rate and review purchased books
- **OAuth Login**: Login with Google (no password hassle)

### Admin Features
- **Admin Dashboard**: Real-time analytics and metrics
- **Book Management**: CRUD operations for books and categories
- **File Upload**: Secure PDF book and cover image uploads
- **Order Management**: Manage customer orders
- **User Management**: Admin panel for user administration
- **Sales Reports**: Detailed analytics and reporting
- **Google OAuth**: OAuth configuration in admin panel

## Architecture Overview

```
                    +-----------------+
                    |   Nginx Gateway  |
                    |     (8000)      |
                    +--------+--------+
                             |
                    +--------v--------+
                    |   API Gateway   |
                    |     (8000)      |
                    +--------+--------+
                             |
        +---------------+        +---------------+
        | Auth Service |        | Payment Service |
        |    (3001)    |        |    (3003)      |
        +---------------+        +---------------+
                            |
                +-----------v-----------+
                |    Book Service      |
                |     (3002)          |
                +---------------------+

+----------------+----------------+
| PostgreSQL      |     Redis       |
|    (5432)       |     (6379)      |
+----------------+----------------+
```


## Tech Stack

### Backend Services
- **Language**: Rust (compiled to native for maximum performance)
- **Web Framework**: Axum 0.8.6 - super fast async web framework
- **Database**: PostgreSQL 16 with connection pooling
- **Cache**: Redis 7 for session management and query caching
- **Authentication**: JWT with refresh token rotation + Google OAuth2
- **File Processing**: Multipart uploads with security scanning
- **Background Jobs**: Tokio Cron Scheduler
- **Payment**: Midtrans gateway integration

### Frontend
- **Language**: Vanilla JavaScript (ES6+)
- **Architecture**: Modular JavaScript with separation of concerns
- **Features**: SPA-like navigation, shopping cart, PDF reader
- **Styling**: CSS3 with responsive design

### DevOps & Infrastructure
- **Containerization**: Docker with multi-stage builds
- **Orchestration**: Docker Compose for development
- **Load Balancing**: Nginx as reverse proxy
- **Database**: Migration system with version control
- **Build System**: Cargo workspace with Makefile automation

### Quick Start
```bash
# Clone repository
git clone https://github.com/Arsysky7/BookStrore.git
cd pdf-bookstore

# Start all services
docker-compose up -d

# Check service health
make health-check

# Access application
# Frontend: http://localhost:8080
# Admin Panel: http://localhost:8081
# API: http://localhost:8000
```
