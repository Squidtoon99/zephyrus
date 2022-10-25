use proc_macro2::{Ident, TokenStream as TokenStream2};
use syn::{parse2, spanned::Spanned, Block, Error, ItemFn, Result, Signature, Type};

/// The implementation of the command macro, this macro modifies the provided function body to allow
/// parsing all function arguments and wraps it into a command struct, registering all command names,
/// types and descriptions.
pub fn button(macro_attrs: TokenStream2, input: TokenStream2) -> Result<TokenStream2> {
    let fun = parse2::<ItemFn>(input)?;

    let ItemFn {
        mut attrs,
        vis,
        mut sig,
        mut block,
    } = fun;

    if sig.inputs.is_empty() {
        // The function must have at least one argument, which must be an `SlashContext`
        return Err(Error::new(
            sig.inputs.span(),
            "Expected at least SlashContext as a parameter",
        ));
    }

    // If we provided a name at macro invocation, use it, if not, use the function's one
    let name = if macro_attrs.is_empty() {
        sig.ident.to_string()
    } else {
        parse2::<syn::LitStr>(macro_attrs)?.value()
    };

    /*
    Set the return type of the function, warning the user if the provided output does not match
    the required one.
    */
    sig.output = parse2(quote::quote!(-> ::zephyrus::prelude::CommandResult))?;

    // The name of the function
    let ident = sig.ident.clone();
    // The name the function will have after macro execution
    let fn_ident = quote::format_ident!("_{}", &sig.ident);
    sig.ident = fn_ident.clone();

    let (context_ident, context_type) = crate::util::get_context_type_and_ident(&sig)?;
    // Get the futurize macro so we can fit the function into a normal fn pointer
    let extract_output = crate::util::get_futurize_macro();
    let button_path = crate::util::get_button_path();

    let args = parse_arguments(&mut sig, &mut block, context_ident, &context_type)?;
    let opts = crate::options::CommandDetails::parse(&mut attrs)?;

    Ok(quote::quote! {
        pub fn #ident() -> #button_path<#context_type> {
            #button_path::new(#fn_ident)
                .name(#name)
                #opts
                #(#args)*
        }

        #[#extract_output]
        #(#attrs)*
        #vis #sig #block
    })
}

/// Prepares the given function to parse the required arguments
pub fn parse_arguments<'a>(
    sig: &mut Signature,
    block: &mut Block,
    ctx_ident: Ident,
    ctx_type: &'a Type,
) -> Result<Vec<crate::argument::Argument<'a>>> {
    use crate::argument::Argument;

    let mut arguments = Vec::new();
    while sig.inputs.len() > 1 {
        arguments.push(Argument::new(
            sig.inputs.pop().unwrap().into_value(),
            ctx_type,
        )?);
    }

    arguments.reverse();

    let (names, types, renames) = (
        arguments.iter().map(|s| &s.name).collect::<Vec<_>>(),
        arguments.iter().map(|s| &s.ty).collect::<Vec<_>>(),
        arguments
            .iter()
            .map(|s| {
                if let Some(renaming) = &s.renaming {
                    renaming.to_owned()
                } else {
                    s.name.to_string()
                }
            })
            .collect::<Vec<_>>(),
    );
    let parse_trait = crate::util::get_parse_trait();

    // The original block of the function
    let b = &block;

    // Modify the block to parse arguments
    *block = parse2(quote::quote! {{
        let (#(#names),*) = {
            #[allow(unused_mut)]
            let mut __options = ::zephyrus::iter::DataIterator::new(
                vec![]
            );

            #(let #names: #types =
                #parse_trait::<#ctx_type>::named_parse(#renames, &#ctx_ident.http_client, &#ctx_ident.data, &mut __options).await?;)*

            if __options.len() > 0 {
                return Err(
                    Box::new(::zephyrus::prelude::ParseError::StructureMismatch("Too many arguments received".to_string()))
                    as Box<dyn std::error::Error + Sync + std::marker::Send>
                );
            }

            (#(#names),*)
        };

        #b
    }})?;

    Ok(arguments)
}
