import sympy


def analyze_expression(expr_str):
    # Preprocess
    clean_expr = expr_str.replace('^', '**')
    
    # Symbol env
    local_dict = {
        'e': sympy.E,
        'pi': sympy.pi,
        'phi': sympy.GoldenRatio,
        'sqrt2': sympy.sqrt(2),
        'ln2': sympy.ln(2)
    }
    
    # Parse
    expr = sympy.parse_expr(clean_expr, local_dict=local_dict)
    
    # Simplify and expand
    simplified_expr = sympy.simplify(expr)
    expanded_expr = sympy.expand(simplified_expr)
    
    # LaTeX generation
    latex_original = sympy.latex(expr, ln_notation=True)
    latex_simplified = sympy.latex(simplified_expr, ln_notation=True)
    latex_expanded = sympy.latex(expanded_expr, ln_notation=True)
    
    # Numeric evaluation
    precision = 64
    numeric_value = simplified_expr.evalf(precision)
    
    return {
        "original": expr,
        "latex_original": latex_original,
        "simplified": simplified_expr,
        "expanded": expanded_expr,
        "latex_simplified": latex_simplified,
        "latex_expanded": latex_expanded,
        "value": numeric_value
    }

if __name__ == "__main__":
    import sys
    
    # Default expression if no command-line input is provided
    raw_expr = "(ln2 * (ln2 ^ (ln2 - ln2)))"
    
    # Use command-line argument if available
    if len(sys.argv) > 1:
        raw_expr = sys.argv[1]
    
    print("Processing...")
    result = analyze_expression(raw_expr)
    
    print("\n[Original]")
    print(result["original"])
    
    print("\n[Simplified]")
    print(result["simplified"])

    print("\n[Expanded]")
    print(result["expanded"])
    
    print("\n[LaTeX (Original)]")
    print(result["latex_original"])
    
    print("\n[LaTeX (Simplified)]")
    print(result["latex_simplified"])

    print("\n[LaTeX (Expanded)]")
    print(result["latex_expanded"])
    
    print("\n[Value]")
    print(result["value"])
