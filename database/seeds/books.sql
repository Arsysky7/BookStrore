-- /pdf-bookstore/database/seeds/books.sql

INSERT INTO books (id, title, author, description, isbn, price, pdf_path, cover_path, file_size_mb, total_pages, language) VALUES
(gen_random_uuid(), 
 'Rust Programming Language', 
 'Steve Klabnik', 
 'Comprehensive guide to Rust programming language with practical examples and best practices.',
 '978-1593278519', 
 89000.00, 
 '/storage/books/rust-programming.pdf', 
 '/storage/covers/rust-programming.jpg', 
 15.5, 
 552, 
 'en'),

(gen_random_uuid(), 
 'JavaScript: The Complete Guide', 
 'Maximilian Schwarzmüller', 
 'Modern JavaScript from basics to advanced concepts including ES6+, DOM manipulation, and frameworks.',
 '978-1491950357', 
 75000.00, 
 '/storage/books/javascript-guide.pdf', 
 '/storage/covers/javascript-guide.jpg', 
 12.8, 
 456, 
 'en'),

(gen_random_uuid(), 
 'Microservices Architecture', 
 'Chris Richardson', 
 'Design patterns and best practices for building scalable microservice applications.',
 '978-1617294549', 
 95000.00, 
 '/storage/books/microservices-arch.pdf', 
 '/storage/covers/microservices-arch.jpg', 
 18.2, 
 624, 
 'en'),

(gen_random_uuid(), 
 'PostgreSQL Up & Running', 
 'Regina Obe', 
 'Essential guide to PostgreSQL database administration and optimization.',
 '978-1449373412', 
 65000.00, 
 '/storage/books/postgresql-guide.pdf', 
 '/storage/covers/postgresql-guide.jpg', 
 9.7, 
 368, 
 'en'),

(gen_random_uuid(), 
 'Digital Marketing Handbook', 
 'Ryan Deiss', 
 'Complete guide to digital marketing strategies, SEO, and social media marketing.',
 '978-1118557068', 
 55000.00, 
 '/storage/books/digital-marketing.pdf', 
 '/storage/covers/digital-marketing.jpg', 
 8.3, 
 298, 
 'id');

INSERT INTO book_categories (book_id, category_id) 
SELECT b.id, c.id FROM books b, categories c 
WHERE (b.title = 'Rust Programming Language' AND c.slug = 'programming')
   OR (b.title = 'JavaScript: The Complete Guide' AND c.slug = 'programming')
   OR (b.title = 'Microservices Architecture' AND c.slug = 'technology')
   OR (b.title = 'PostgreSQL Up & Running' AND c.slug = 'programming')
   OR (b.title = 'Digital Marketing Handbook' AND c.slug = 'marketing');