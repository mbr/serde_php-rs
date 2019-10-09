# PHP serialization support for serde

PHP uses a custom serialization format through its `serialize` and `unserialize` methods. This crate adds partial support for this format using serde.

An overview of the format can be seen at https://stackoverflow.com/questions/14297926/structure-of-a-serialized-php-string, details are available at http://www.phpinternalsbook.com/php5/classes_objects/serialization.html.

## What is supported?

* Basic and compound types:

  | PHP type                | Rust type                                             |
  | ---                     | ---                                                   |
  | boolean                 | `bool`                                                |
  | integer                 | `i64` (automatic conversion to other types supported) |
  | float                   | `f64` (automatic conversion to `f32` supported)       |
  | strings                 | `Vec<u8>` (PHP strings are not UTF8)                  |
  | null                    | decoded as `None`                                     |
  | array (non-associative) | tuple `struct`s or `Vec<_>`                           |
  | array (associative)     | regular `struct`s or `HashMap<_, _>`                  |

* Rust `String`s are transparently UTF8-converted to PHP bytestrings.

https://stackoverflow.com/questions/14297926/structure-of-a-serialized-php-string

## What is missing?

* PHP objects
* Out-of-order numeric arrays
* Non-string/numeric array keys, except when deserializing into a `HashMap`
* Mixed arrays. Array keys are assumed to always have the same key type
  (Note: If this is required, consider extending this library with a variant type).

## Example use

Given an example data structure storing a session token using the following PHP code

```php
<?php
$serialized = serialize(array("user", "", array()));
echo($serialized);
```

and thus the following output

```
a:3:{i:0;s:4:"user";i:1;s:0:"";i:2;a:0:{}}
```

, the data can be reconstructed using the following rust code:

```rust
use serde::Deserialize;
use serde_php::from_bytes;

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct UserData(Vec<u8>, Vec<u8>, SubData);

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct SubData();

let input = br#"a:3:{i:0;s:4:"user";i:1;s:0:"";i:2;a:0:{}}"#;
assert_eq!(
    from_bytes::<Data>(input).unwrap(),
    Data(b"user".to_vec(), b"".to_vec(), SubData())
);
```

Likewise, structs are supported as well, if the PHP arrays use keys:

```php
<?php
$serialized = serialize(
    array("foo" => true,
          "bar" => "xyz",
          "sub" => array("x" => 42))
);
echo($serialized);
```

In Rust:

```rust
#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Outer {
    foo: bool,
    bar: String,
    sub: Inner,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Inner {
    x: i64,
}

let input = br#"a:3:{s:3:"foo";b:1;s:3:"bar";s:3:"xyz";s:3:"sub";\
                a:1:{s:1:"x";i:42;}}"#;
let expected = Outer {
    foo: true,
    bar: "xyz".to_owned(),
    sub: Inner { x: 42 },
};

assert_eq!(from_bytes(input), Ok(expected));
```

### Optional values

Missing values can be left optional, as in this example:

```php
<?php
$location_a = array();
$location_b = array("province" => "Newfoundland and Labrador, CA");
$location_c = array("postalcode" => "90002",
                    "country" => "United States of America");
echo(serialize($location_a) . "\n");
echo(serialize($location_b) . "\n");
# -> a:1:{s:8:"province";s:29:"Newfoundland and Labrador, CA";}
echo(serialize($location_c) . "\n");
# -> a:2:{s:10:"postalcode";s:5:"90002";s:7:"country";
#         s:24:"United States of America";}
```

The following declaration of `Location` will be able to parse all three example inputs.

```rust
#[derive(Debug, Deserialize, Eq, PartialEq)]
struct Location {
    province: Option<String>,
    postalcode: Option<String>,
    country: Option<String>,
}
```
