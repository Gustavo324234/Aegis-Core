#!/usr/bin/env python3
"""
Aegis OS — Script de Fine-Tuning QLoRA para el Asistente Local
Permite entrenar modelos open-weights (como Qwen 2.5, Llama 3.1 o Phi-4) 
optimizando el formateo de herramientas y el tono conversacional de voz.
"""

import os
import argparse
import torch
from datasets import load_dataset
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    BitsAndBytesConfig,
    TrainingArguments,
    TrainerCallback
)
from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training
from trl import SFTTrainer

class PrintLossCallback(TrainerCallback):
    """Callback para monitorear el progreso del entrenamiento imprimiendo la pérdida."""
    def on_log(self, args, state, control, logs=None, **kwargs):
        if logs and "loss" in logs:
            print(f"Paso {state.global_step}: Pérdida (Loss) = {logs['loss']:.4f}")

def parse_args():
    parser = argparse.ArgumentParser(description="Entrenar el modelo asistente de Aegis OS.")
    parser.add_argument(
        "--model_id",
        type=str,
        default="Qwen/Qwen2.5-7B-Instruct",
        help="Model ID de Hugging Face a utilizar como base (ej: Qwen/Qwen2.5-7B-Instruct, microsoft/Phi-4-mini-instruct, meta-llama/Llama-3.1-8B-Instruct)."
    )
    parser.add_argument(
        "--dataset_path",
        type=str,
        default="dataset_template.jsonl",
        help="Ruta al archivo dataset.jsonl con los datos de entrenamiento."
    )
    parser.add_argument(
        "--output_dir",
        type=str,
        default="./aegis-assistant-lora",
        help="Carpeta de salida para guardar los pesos del adaptador LoRA."
    )
    parser.add_argument(
        "--epochs",
        type=int,
        default=3,
        help="Número de épocas de entrenamiento."
    )
    parser.add_argument(
        "--batch_size",
        type=int,
        default=2,
        help="Batch size por dispositivo (reducir si hay fallas de memoria VRAM)."
    )
    parser.add_argument(
        "--learning_rate",
        type=float,
        default=2e-4,
        help="Tasa de aprendizaje (learning rate)."
    )
    parser.add_argument(
        "--max_seq_length",
        type=int,
        default=2048,
        help="Longitud máxima de contexto para el entrenamiento."
    )
    return parser.parse_args()

def main():
    args = parse_args()
    print("==========================================================")
    print(" Iniciando Pipeline de Fine-Tuning para Aegis OS")
    print(f" Modelo Base: {args.model_id}")
    print(f" Dataset:      {args.dataset_path}")
    print(f" Directorio:   {args.output_dir}")
    print("==========================================================")

    # 1. Cargar el Tokenizador
    print("\n[1/6] Cargando tokenizador...")
    tokenizer = AutoTokenizer.from_pretrained(args.model_id, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    # Añadir tokens de control específicos de Aegis OS
    special_tokens = ["<aegis_sys_call>", "</aegis_sys_call>", "<siren_audio>"]
    tokenizer.add_special_tokens({"additional_special_tokens": special_tokens})

    # 2. Configurar la cuantización a 4 bits para QLoRA
    print("\n[2/6] Configurando cuantización a 4-bits...")
    bnb_config = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_use_double_quant=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16
    )

    # 3. Cargar el Modelo Base
    print("\n[3/6] Descargando y cargando el modelo base en memoria...")
    model = AutoModelForCausalLM.from_pretrained(
        args.model_id,
        quantization_config=bnb_config,
        device_map="auto",
        trust_remote_code=True
    )
    
    # Redimensionar embeddings para soportar los nuevos tokens especiales
    model.resize_token_embeddings(len(tokenizer))
    
    # Preparar el modelo para entrenamiento de bits (habilita gradientes y congelamiento)
    model = prepare_model_for_kbit_training(model)

    # 4. Configurar LoraConfig (PEFT)
    print("\n[4/6] Configurando estructura de adaptadores LoRA...")
    # Identificar módulos clave del modelo según la familia (Qwen, Llama, etc.)
    target_modules = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]
    
    lora_config = LoraConfig(
        r=16,
        lora_alpha=32,
        target_modules=target_modules,
        lora_dropout=0.05,
        bias="none",
        task_type="CAUSAL_LM"
    )

    model = get_peft_model(model, lora_config)
    model.print_trainable_parameters()

    # 5. Cargar y procesar el Dataset
    print("\n[5/6] Cargando datos de entrenamiento...")
    if not os.path.exists(args.dataset_path):
        raise FileNotFoundError(f"No se encontró el dataset en la ruta: {args.dataset_path}")
    
    dataset = load_dataset("json", data_files=args.dataset_path, split="train")

    # Definir la función de mapeo de chat a string
    def format_prompts(batch):
        formatted_texts = []
        for messages in batch["messages"]:
            # Aplicar la plantilla de chat del tokenizador
            text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=False)
            formatted_texts.append(text)
        return {"text": formatted_texts}

    dataset = dataset.map(format_prompts, batched=True)

    # 6. Configurar Argumentos de Entrenamiento
    print("\n[6/6] Preparando configuración de entrenamiento...")
    training_args = TrainingArguments(
        output_dir=args.output_dir,
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=4,
        warmup_steps=10,
        max_steps=-1,
        num_train_epochs=args.epochs,
        learning_rate=args.learning_rate,
        fp16=not torch.cuda.is_bf16_supported(),
        bf16=torch.cuda.is_bf16_supported(),
        logging_steps=10,
        optim="paged_adamw_8bit",
        save_strategy="epoch",
        remove_unused_columns=False,
        report_to="none"  # Desactivar telemetría externa por defecto
    )

    trainer = SFTTrainer(
        model=model,
        train_dataset=dataset,
        dataset_text_field="text",
        max_seq_length=args.max_seq_length,
        tokenizer=tokenizer,
        args=training_args,
        callbacks=[PrintLossCallback()]
    )

    print("\n==========================================================")
    print(" INICIANDO ENTRENAMIENTO...")
    print("==========================================================")
    
    trainer.train()

    print("\n==========================================================")
    print(" ENTRENAMIENTO COMPLETADO")
    print("==========================================================")

    # Guardar el modelo entrenado (sólo los adaptadores) y el tokenizador
    print(f"\nGuardando adaptadores LoRA en: {args.output_dir}")
    trainer.model.save_pretrained(args.output_dir)
    tokenizer.save_pretrained(args.output_dir)
    print("¡Proceso finalizado con éxito!")

if __name__ == "__main__":
    main()
