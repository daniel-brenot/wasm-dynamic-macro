use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn, Signature, ForeignItemFn, parse::Parse, token, Token, Visibility, ForeignItem};
use quote::quote;

struct ForeignItemFns {
    pub fns: Vec<ForeignItemFn>,
}

impl Parse for ForeignItemFns {
    fn parse(content: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut items: Vec<ForeignItem> = Vec::new();
        while !content.is_empty() {
            items.push(content.parse()?);
        }
        panic!("Past the while loop with length {}", items.len());
        let mut fns: Vec<ForeignItemFn> = Vec::new();
        for item in items.iter() {
            if let ForeignItem::Fn(func) = item {
                fns.push(func.clone());
            }
        }
        return Ok(ForeignItemFns { fns: Vec::new() });

    }
}

#[proc_macro_attribute]
pub fn host_dynamic(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let ItemFn {
        attrs: _,
        vis,
        sig,
        block
    } = parse_macro_input!(input as ItemFn);
    let Signature {
        unsafety,
        ident,
        inputs,
        ..
    } = sig;
    // TODO check if this can be skipped for non dynamic data
    TokenStream::from(quote!{
        #vis #unsafety fn #ident (#inputs) -> i32 {
            use bytevec::ByteEncodable;
            let return_value = #block;
            if let Ok(encoded_return) = return_value.encode::<u32>() {
                env.data_mut().last_result = encoded_return;
                return env.data().last_result.len() as _
            }
            return -1;
        }
    })
}

/// Takes a list of function definitions and creates safe
/// versions of them for communicating across the wasm barrier
#[proc_macro]
pub fn guest_dynamic(item: TokenStream) -> TokenStream {
    // list of function definitions
    let definitions = parse_macro_input!(item as ForeignItemFns).fns;
    let new_definitions = definitions.iter().map(|func| {
        let Signature {
            ident,
            inputs,
            output,
            ..
        } = &func.sig;
        let mut out_stream = quote!();
        while let syn::ReturnType::Type(_ar, rt) = output {
            let arg_names: Vec<syn::Ident> = inputs.iter().filter_map(|f|{ 
                if let syn::FnArg::Typed(arg) = f {
                    if let syn::Pat::Ident(ident) = &*arg.pat {
                        return Some(ident.ident.clone());
                    }
                }
                None
            }).collect();
            out_stream.extend(quote! {
                pub fn #ident(#inputs) -> Option<#rt> {
                    // Internal unsafe function that will be wrapped
                    extern "C" { fn #ident(#inputs) -> isize; }
                    let rval = unsafe { #ident(#(#arg_names),*) };
                    if rval < 0 { return None; }
                    Some(unsafe { memcpy(rval as u32) })
                }
            });
        }
        return out_stream;
    });
    return TokenStream::from(quote! {
        unsafe fn memcpy<T>(size: u32) -> T where T: bytevec::ByteDecodable {
            extern "C" { fn get_mem(addr:u32) -> u8; }
            let mut ret_bytes = Vec::<u8>::with_capacity(size as usize);
            for i in 0..size {
                ret_bytes.push(unsafe { get_mem(i) });
            }
            T::decode::<u32>(&ret_bytes).unwrap()
        }
        #(#new_definitions)*
    });
    
}