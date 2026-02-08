<script setup lang="ts">
import { useFetch } from '@vueuse/core';



const scanBans = async () => {
    const { data, error } = await useFetch(`/api/scan-all-stickers-for-bans`).post()
    console.log(data, error)
    if (error.value) {
      alert(error.value)
    }
}

</script>

<template>
  <div class="about">
    <h1>This is an about page</h1>
  </div>
  <v-btn @click="scanBans">
    scan all stickers for bans
  </v-btn>
</template>

<style>

</style>
